/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Plan 61 bzlmod graph identities and guardrail counters.
//!
//! These types are intentionally inert in 61.1. They name the DICE-owned values
//! the migration will switch to, while the current legacy startup path remains
//! the behavioral authority. Existing code can use the counters immediately to
//! prove when legacy paths compute, replay, or materialize bzlmod state.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_error::BuckErrorContext;
use slug_util::late_binding::LateBinding;

use crate::BzlmodRepoMapping;
use crate::RegisteredToolchain;
use crate::cache::ModuleCache;
use crate::extensions::AggregatedExtension;
use crate::extensions::aggregate_extensions_with_policy;
use crate::lockfile::Lockfile;
use crate::lockfile::LockfileMode;
use crate::lockfile::lockfile_path;
use crate::parser::ModuleFileInputDigest;
use crate::parser::validate_parsed_root_extension_repo_directives;
use crate::registry::SourceInfo;
use crate::repo_spec::RepoSpec;
use crate::resolution::ModuleKey;
use crate::resolution::ModuleSource;
use crate::resolution::MvsResolver;
use crate::resolution::ResolvedGraph;
use crate::resolution::ResolvedModuleInfo;
use crate::resolution::parse_allowed_yanked_versions;
use crate::types::Module;
use crate::types::Override;
use crate::types::ParsedModuleFile;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct WorkspaceId {
    pub canonical_project_root: Arc<PathBuf>,
    pub output_base: Arc<PathBuf>,
    pub stable_hash: Arc<str>,
}

impl WorkspaceId {
    pub fn new(canonical_project_root: PathBuf, output_base: PathBuf) -> Self {
        let stable_hash = workspace_hash(&canonical_project_root, &output_base);
        Self {
            canonical_project_root: Arc::new(canonical_project_root),
            output_base: Arc::new(output_base),
            stable_hash: Arc::from(stable_hash.as_str()),
        }
    }

    pub fn stable_hash(&self) -> &str {
        &self.stable_hash
    }

    pub fn no_project_sentinel() -> Self {
        Self::new(PathBuf::new(), PathBuf::from("buck-out/v2"))
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::new(project_root.clone(), project_root.join("buck-out/v2"))
    }
}

fn workspace_hash(canonical_project_root: &Path, output_base: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_project_root.to_string_lossy().as_bytes());
    hasher.update([0]);
    hasher.update(output_base.to_string_lossy().as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct BzlmodWorkspaceKey {
    pub canonical_project_root: Arc<PathBuf>,
    pub output_base: Arc<PathBuf>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "BzlmodCommandPolicyKey({}, {})",
    workspace_id.canonical_project_root.display(),
    bzlmod_flags_digest
)]
pub struct BzlmodCommandPolicyKey {
    pub workspace_id: WorkspaceId,
    pub bazel_release_id: Arc<str>,
    pub starlark_semantics_digest: Arc<str>,
    pub bzlmod_flags_digest: Arc<str>,
    pub lockfile_mode: Arc<str>,
    pub registry_config_digest: Arc<str>,
    pub repository_cache_config_digest: Arc<str>,
    pub network_policy_digest: Arc<str>,
    pub repo_env_digest: Arc<str>,
    pub nonstrict_repo_env_digest: Arc<str>,
    pub ignore_dev_dependency: bool,
    pub allow_yanked_versions_digest: Arc<str>,
    pub bazel_compatibility_policy_digest: Arc<str>,
    pub isolated_extension_usages: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodCommandPolicyValue {
    pub workspace_id: WorkspaceId,
    pub digest: Arc<str>,
    pub repo_env_digest: Arc<str>,
    pub lockfile_mode: Arc<str>,
    pub ignore_dev_dependency: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct BzlmodResolutionOptions {
    pub lockfile_mode: LockfileMode,
    pub ignore_dev_dependency: bool,
    pub allow_yanked_versions_env: Option<String>,
    pub allow_yanked_versions_flags: Vec<String>,
    pub hidden_lockfile_path: Option<PathBuf>,
    pub repo_env: BTreeMap<String, String>,
    pub repo_env_digest: String,
}

const BZLMOD_BAZEL_RELEASE_ID: &str = "bazel-9.0.1";
const BZLMOD_STARLARK_SEMANTICS_DIGEST: &str = "slug-bazel9-starlark-semantics-v1";
const BZLMOD_DEFAULT_REGISTRY_CONFIG_DIGEST: &str = "default-registry-config";
const BZLMOD_DEFAULT_REPOSITORY_CACHE_CONFIG_DIGEST: &str = "default-repository-cache-config";
const BZLMOD_DEFAULT_NETWORK_POLICY_DIGEST: &str = "default-network-policy";
const BZLMOD_DEFAULT_NONSTRICT_REPO_ENV_DIGEST: &str = "empty-nonstrict-repo-env";
const BZLMOD_DEFAULT_BAZEL_COMPATIBILITY_POLICY_DIGEST: &str = "default-bazel-compatibility-policy";

impl BzlmodResolutionOptions {
    pub fn policy_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{:?}", self.lockfile_mode).as_bytes());
        hasher.update([0]);
        hasher.update([u8::from(self.ignore_dev_dependency)]);
        hasher.update([0]);
        if let Some(value) = &self.allow_yanked_versions_env {
            hasher.update(value.as_bytes());
        }
        hasher.update([0]);
        for value in &self.allow_yanked_versions_flags {
            hasher.update(value.as_bytes());
            hasher.update([0]);
        }
        hasher.update(self.repo_env_digest.as_bytes());
        hasher.update([0]);
        if let Some(value) = &self.hidden_lockfile_path {
            hasher.update(value.to_string_lossy().as_bytes());
        }
        hasher.update([0]);
        hex::encode(hasher.finalize())
    }

    pub fn command_policy_key(&self, workspace_id: WorkspaceId) -> BzlmodCommandPolicyKey {
        BzlmodCommandPolicyKey {
            workspace_id,
            bazel_release_id: Arc::from(BZLMOD_BAZEL_RELEASE_ID),
            starlark_semantics_digest: Arc::from(BZLMOD_STARLARK_SEMANTICS_DIGEST),
            bzlmod_flags_digest: Arc::from(self.policy_digest().as_str()),
            lockfile_mode: Arc::from(format!("{:?}", self.lockfile_mode).as_str()),
            registry_config_digest: Arc::from(BZLMOD_DEFAULT_REGISTRY_CONFIG_DIGEST),
            repository_cache_config_digest: Arc::from(
                BZLMOD_DEFAULT_REPOSITORY_CACHE_CONFIG_DIGEST,
            ),
            network_policy_digest: Arc::from(BZLMOD_DEFAULT_NETWORK_POLICY_DIGEST),
            repo_env_digest: Arc::from(self.repo_env_digest.as_str()),
            nonstrict_repo_env_digest: Arc::from(BZLMOD_DEFAULT_NONSTRICT_REPO_ENV_DIGEST),
            ignore_dev_dependency: self.ignore_dev_dependency,
            allow_yanked_versions_digest: Arc::from(
                allow_yanked_versions_digest(
                    self.allow_yanked_versions_env.as_deref(),
                    &self.allow_yanked_versions_flags,
                )
                .as_str(),
            ),
            bazel_compatibility_policy_digest: Arc::from(
                BZLMOD_DEFAULT_BAZEL_COMPATIBILITY_POLICY_DIGEST,
            ),
            isolated_extension_usages: false,
        }
    }
}

pub fn bzlmod_resolved_graph_digest(graph: &ResolvedGraph) -> String {
    fn update(hasher: &mut Sha256, value: &str) {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }

    let mut hasher = Sha256::new();
    update(&mut hasher, "bzlmod-resolved-module-graph-v1");

    let mut selected_versions: Vec<_> = graph.selected_versions.iter().collect();
    selected_versions.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (name, version) in selected_versions {
        update(&mut hasher, "selected");
        update(&mut hasher, name);
        update(&mut hasher, version);
    }

    for module_name in &graph.resolution_order {
        update(&mut hasher, "order");
        update(&mut hasher, module_name);
    }

    let mut modules: Vec<_> = graph.modules.iter().collect();
    modules.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (name, info) in modules {
        update(&mut hasher, "module");
        update(&mut hasher, name);
        update(&mut hasher, &info.name);
        update(&mut hasher, &info.version);
        update(&mut hasher, &info.compatibility_level.to_string());
        let mut deps: Vec<_> = info.dependencies.iter().collect();
        deps.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (dep, version) in deps {
            update(&mut hasher, "dep");
            update(&mut hasher, dep);
            update(&mut hasher, version);
        }
        update(&mut hasher, &format!("{:?}", info.source));
        update(
            &mut hasher,
            info.source_path
                .as_ref()
                .map(|path| path.to_string_lossy())
                .as_deref()
                .unwrap_or(""),
        );
    }

    let mut registry_hashes: Vec<_> = graph.registry_file_hashes.iter().collect();
    registry_hashes.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (url, digest) in registry_hashes {
        update(&mut hasher, "registry");
        update(&mut hasher, url);
        update(&mut hasher, digest);
    }

    let mut yanked_versions: Vec<_> = graph.selected_yanked_versions.iter().collect();
    yanked_versions.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (module, reason) in yanked_versions {
        update(&mut hasher, "yanked");
        update(&mut hasher, module);
        update(&mut hasher, reason);
    }

    hex::encode(hasher.finalize())
}

pub fn allow_yanked_versions_digest(from_env: Option<&str>, from_flags: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"allow-yanked-versions-policy-v1");
    hasher.update([0]);
    if let Some(value) = from_env {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    hasher.update([0]);
    for value in from_flags {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}

#[async_trait]
impl Key for BzlmodCommandPolicyKey {
    type Value = slug_error::Result<Arc<BzlmodCommandPolicyValue>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Ok(Arc::new(BzlmodCommandPolicyValue {
            workspace_id: self.workspace_id.clone(),
            digest: Arc::from(bzlmod_command_policy_digest(self).as_str()),
            repo_env_digest: self.repo_env_digest.clone(),
            lockfile_mode: self.lockfile_mode.clone(),
            ignore_dev_dependency: self.ignore_dev_dependency,
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

fn bzlmod_command_policy_digest(key: &BzlmodCommandPolicyKey) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"bzlmod-command-policy-v1");
    update_digest_str(&mut hasher, key.workspace_id.stable_hash());
    update_digest_str(&mut hasher, &key.bazel_release_id);
    update_digest_str(&mut hasher, &key.starlark_semantics_digest);
    update_digest_str(&mut hasher, &key.bzlmod_flags_digest);
    update_digest_str(&mut hasher, &key.lockfile_mode);
    update_digest_str(&mut hasher, &key.registry_config_digest);
    update_digest_str(&mut hasher, &key.repository_cache_config_digest);
    update_digest_str(&mut hasher, &key.network_policy_digest);
    update_digest_str(&mut hasher, &key.repo_env_digest);
    update_digest_str(&mut hasher, &key.nonstrict_repo_env_digest);
    hasher.update([u8::from(key.ignore_dev_dependency)]);
    hasher.update([0]);
    update_digest_str(&mut hasher, &key.allow_yanked_versions_digest);
    update_digest_str(&mut hasher, &key.bazel_compatibility_policy_digest);
    hasher.update([u8::from(key.isolated_extension_usages)]);
    hasher.update([0]);
    hex::encode(hasher.finalize())
}

fn update_digest_str(hasher: &mut Sha256, value: &str) {
    hasher.update(value.as_bytes());
    hasher.update([0]);
}

pub fn repo_env_policy_digest(repo_env: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"repo-env-policy-v1");
    hasher.update([0]);
    for (name, value) in repo_env {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}

pub(crate) const INJECTED_BZLMOD_PROJECTION_DIGEST: &str = "injected-bzlmod-projection";
pub(crate) const EMPTY_BZLMOD_CELL_GRAPH_DIGEST: &str = "empty-bzlmod-cell-graph";

/// DICE-owned root `MODULE.bazel` read/parse result.
#[derive(Clone, Debug, Allocative)]
pub struct RootModuleFileValue {
    pub path: Arc<PathBuf>,
    pub input_digest: Option<String>,
    pub input_count: usize,
    pub parsed: Option<ParsedModuleFile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct LocalOverrideModuleInputsValue {
    pub digest: String,
    pub parsed_modules: Vec<(String, ParsedModuleFile)>,
    pub missing_module_dirs: Vec<String>,
    pub has_bazel_deps: bool,
    pub has_extension_usages: bool,
    pub has_repo_rule_invocations: bool,
    pub has_git_overrides: bool,
    pub has_untracked_inputs: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct NonRegistryOverrideModuleInputsValue {
    pub digest: String,
    pub parsed_modules: Vec<(String, ParsedModuleFile)>,
    pub module_dirs: Vec<(String, PathBuf)>,
    pub has_inputs: bool,
    pub has_untracked_inputs: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct NonRegistryOverrideModuleInput {
    pub module_name: String,
    pub module_dir: PathBuf,
    pub source: NonRegistryOverrideModuleSource,
}

impl NonRegistryOverrideModuleInput {
    pub fn kind(&self) -> &'static str {
        self.source.kind()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub enum NonRegistryOverrideModuleSource {
    Git {
        remote: String,
        commit: String,
        shallow_since: Option<String>,
        patches: Vec<String>,
        patch_strip: u32,
    },
    Archive {
        urls: Vec<String>,
        integrity: Option<String>,
        strip_prefix: Option<String>,
        patches: Vec<String>,
        patch_strip: u32,
    },
}

impl NonRegistryOverrideModuleSource {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Git { .. } => "git override",
            Self::Archive { .. } => "archive override",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct RegistryFileInputsValue {
    pub digest: String,
    pub has_inputs: bool,
    pub cache_safe: bool,
    pub has_untracked_inputs: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct NonRootModuleFilesValue {
    pub digest: String,
    pub parsed_modules: Vec<(String, ParsedModuleFile)>,
    pub has_untracked_inputs: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct NonRootModuleFileInput {
    pub module_key: String,
    pub module_bazel_path: PathBuf,
}

#[derive(Clone, Debug, Allocative)]
pub struct BzlmodResolvedGraphSourceInputsValue {
    pub root_module_file: Arc<RootModuleFileValue>,
    pub lockfile_inputs: Arc<BzlmodLockfileInputsValue>,
    pub local_override_inputs: Arc<LocalOverrideModuleInputsValue>,
    pub non_registry_override_inputs: Arc<NonRegistryOverrideModuleInputsValue>,
    pub registry_file_inputs: Arc<RegistryFileInputsValue>,
    pub override_patch_inputs: Arc<crate::OverridePatchInputs>,
}

impl BzlmodResolvedGraphSourceInputsValue {
    pub fn identity_digest_with_key<K: Hash>(&self, key: &K) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        self.root_module_file.path.hash(&mut hasher);
        self.root_module_file.input_digest.hash(&mut hasher);
        self.lockfile_inputs.hash_identity(&mut hasher);
        self.local_override_inputs.digest.hash(&mut hasher);
        self.non_registry_override_inputs.digest.hash(&mut hasher);
        self.registry_file_inputs.digest.hash(&mut hasher);
        self.override_patch_inputs.digest.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

#[derive(Clone, Debug, Display, Allocative)]
#[display(
    "BzlmodResolvedModuleGraphKey({}, {})",
    workspace_id.canonical_project_root.display(),
    workspace_id.stable_hash()
)]
pub struct BzlmodResolvedModuleGraphKey {
    pub workspace_id: WorkspaceId,
    pub options: BzlmodResolutionOptions,
    pub validate_root_extension_repo_directives: bool,
}

#[derive(Clone, Debug, PartialEq, Allocative)]
pub struct BzlmodResolvedModuleGraphValue {
    pub lockfile_inputs: Arc<BzlmodLockfileInputsValue>,
    pub outputs: Arc<Option<BzlmodResolvedGraphOutputsValue>>,
}

#[async_trait]
pub trait BzlmodCleanGraphIo: Send + Sync + 'static {
    async fn compute_source_inputs(
        &self,
        key: &BzlmodResolvedModuleGraphKey,
        ctx: &mut DiceComputations<'_>,
    ) -> slug_error::Result<BzlmodResolvedGraphSourceInputsValue>;

    async fn compute_non_root_module_files(
        &self,
        key: &BzlmodResolvedModuleGraphKey,
        ctx: &mut DiceComputations<'_>,
        inputs: Vec<NonRootModuleFileInput>,
        root_module_name: &str,
    ) -> slug_error::Result<Arc<NonRootModuleFilesValue>>;

    async fn compute_lockfile_content(
        &self,
        workspace_id: &WorkspaceId,
        kind: LockfileContentKind,
        path: Arc<PathBuf>,
        ctx: &mut DiceComputations<'_>,
    ) -> slug_error::Result<Arc<LockfileContentValue>>;
}

pub static BZLMOD_CLEAN_GRAPH_IO_IMPL: LateBinding<&'static dyn BzlmodCleanGraphIo> =
    LateBinding::new("BZLMOD_CLEAN_GRAPH_IO_IMPL");

impl PartialEq for BzlmodResolvedModuleGraphKey {
    fn eq(&self, other: &Self) -> bool {
        self.workspace_id == other.workspace_id
            && self.options == other.options
            && self.validate_root_extension_repo_directives
                == other.validate_root_extension_repo_directives
    }
}

impl Eq for BzlmodResolvedModuleGraphKey {}

impl std::hash::Hash for BzlmodResolvedModuleGraphKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.workspace_id.hash(state);
        self.options.hash(state);
        self.validate_root_extension_repo_directives.hash(state);
    }
}

#[async_trait]
impl Key for BzlmodResolvedModuleGraphKey {
    type Value = slug_error::Result<Arc<BzlmodResolvedModuleGraphValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        compute_bzlmod_resolved_module_graph(self, ctx).await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => {
                if !x.lockfile_inputs.identity_eq(&y.lockfile_inputs) {
                    return false;
                }
                match (x.outputs.as_ref(), y.outputs.as_ref()) {
                    (Some(x), Some(y)) => {
                        x.graph_digest == y.graph_digest
                            && x.cell_graph_resolution_digest == y.cell_graph_resolution_digest
                            && x.module_versions == y.module_versions
                            && x.resolution_facts == y.resolution_facts
                            && x.registered_toolchains == y.registered_toolchains
                            && x.registered_execution_platforms == y.registered_execution_platforms
                            && x.extension_aggregations == y.extension_aggregations
                            && x.repo_mappings == y.repo_mappings
                            && x.cell_graph == y.cell_graph
                    }
                    (None, None) => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        match x {
            Ok(value) => !value.lockfile_inputs.has_untracked_inputs(),
            Err(_) => false,
        }
    }
}

#[derive(Debug)]
pub struct ResolvedGraphWithModuleFileInputs {
    pub graph: ResolvedGraph,
    pub parsed_modules: Vec<(String, ParsedModuleFile)>,
    pub non_root_module_file_inputs: Vec<NonRootModuleFileInput>,
}

#[derive(Debug)]
pub struct BzlmodResolvedGraphProjectionValues {
    pub module_versions: BzlmodModuleVersionsDataValue,
    pub resolution_facts: BzlmodResolutionFactsValue,
    pub registered_toolchains: RegisteredToolchainsDataValue,
    pub registered_execution_platforms: RegisteredExecutionPlatformsDataValue,
    pub extension_aggregations: BzlmodExtensionAggregationsDataValue,
}

pub fn local_overrides_from_root_module(
    root_module_file: &RootModuleFileValue,
    ignore_dev_dependency: bool,
) -> Vec<(String, String)> {
    root_module_file
        .parsed
        .as_ref()
        .map(|parsed| {
            active_root_overrides(&parsed.module, ignore_dev_dependency)
                .iter()
                .filter_map(|override_| match override_ {
                    Override::LocalPath(local) => {
                        Some((local.module_name.clone(), local.path.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub fn active_root_overrides(module: &Module, ignore_dev_dependency: bool) -> Vec<Override> {
    if !ignore_dev_dependency {
        return module.overrides.clone();
    }

    let ignored_root_dev_deps: HashSet<_> = module
        .bazel_deps
        .iter()
        .filter(|dep| dep.dev_dependency)
        .map(|dep| dep.name.clone())
        .collect();
    module
        .overrides
        .iter()
        .filter(|override_| match override_ {
            Override::LocalPath(local) => !ignored_root_dev_deps.contains(&local.module_name),
            Override::Git(git) => !ignored_root_dev_deps.contains(&git.module_name),
            Override::Archive(archive) => !ignored_root_dev_deps.contains(&archive.module_name),
            _ => true,
        })
        .cloned()
        .collect()
}

pub fn override_patch_labels_from_root_module(
    root_module_file: &RootModuleFileValue,
    ignore_dev_dependency: bool,
) -> (Option<String>, Vec<String>) {
    let Some(parsed) = &root_module_file.parsed else {
        return (None, Vec::new());
    };
    override_patch_labels_from_module(&parsed.module, ignore_dev_dependency)
}

pub fn override_patch_labels_from_module(
    module: &Module,
    ignore_dev_dependency: bool,
) -> (Option<String>, Vec<String>) {
    let main_repo_name = module
        .repo_name
        .clone()
        .or_else(|| Some(module.name.clone()));
    let mut labels = BTreeSet::new();
    for override_ in active_root_overrides(module, ignore_dev_dependency) {
        match override_ {
            Override::SingleVersion(single) => {
                labels.extend(single.patches);
            }
            Override::Git(git) => {
                labels.extend(git.patches);
            }
            Override::Archive(archive) => {
                labels.extend(archive.patches);
            }
            _ => {}
        }
    }
    (main_repo_name, labels.into_iter().collect())
}

pub fn non_registry_override_module_inputs_from_root_module(
    root_module_file: &RootModuleFileValue,
    ignore_dev_dependency: bool,
    override_patch_inputs: &crate::OverridePatchInputs,
) -> slug_error::Result<Vec<NonRegistryOverrideModuleInput>> {
    let Some(parsed) = &root_module_file.parsed else {
        return Ok(Vec::new());
    };
    let active_overrides = active_root_overrides(&parsed.module, ignore_dev_dependency);
    if !active_overrides
        .iter()
        .any(|override_| matches!(override_, Override::Git(_) | Override::Archive(_)))
    {
        return Ok(Vec::new());
    }
    let cache = ModuleCache::new()?;
    let mut inputs = Vec::new();
    for override_ in &active_overrides {
        match override_ {
            Override::Git(git) => {
                let patch_digest =
                    crate::fetch::SourceFetcher::local_override_patch_digest_with_inputs(
                        &git.patches,
                        git.patch_strip,
                        override_patch_inputs,
                    )?;
                inputs.push(NonRegistryOverrideModuleInput {
                    module_name: git.module_name.clone(),
                    module_dir: cache
                        .git_override_dir_with_patch_digest(git, patch_digest.as_deref()),
                    source: NonRegistryOverrideModuleSource::Git {
                        remote: git.remote.clone(),
                        commit: git.commit.clone(),
                        shallow_since: git.shallow_since.clone(),
                        patches: git.patches.clone(),
                        patch_strip: git.patch_strip,
                    },
                });
            }
            Override::Archive(archive) => {
                let patch_digest =
                    crate::fetch::SourceFetcher::local_override_patch_digest_with_inputs(
                        &archive.patches,
                        archive.patch_strip,
                        override_patch_inputs,
                    )?;
                inputs.push(NonRegistryOverrideModuleInput {
                    module_name: archive.module_name.clone(),
                    module_dir: cache
                        .archive_override_dir_with_patch_digest(archive, patch_digest.as_deref()),
                    source: NonRegistryOverrideModuleSource::Archive {
                        urls: archive.urls.clone(),
                        integrity: archive.integrity.clone(),
                        strip_prefix: archive.strip_prefix.clone(),
                        patches: archive.patches.clone(),
                        patch_strip: archive.patch_strip,
                    },
                });
            }
            _ => {}
        }
    }
    Ok(inputs)
}

pub async fn materialize_non_registry_override_module_input(
    input: &NonRegistryOverrideModuleInput,
    override_patch_inputs: &crate::OverridePatchInputs,
) -> slug_error::Result<()> {
    let complete_marker = input.module_dir.join(".complete");
    if complete_marker.exists() {
        tracing::debug!(
            "Using cached {} source for {} at {:?}",
            input.kind(),
            input.module_name,
            input.module_dir
        );
        return Ok(());
    }

    if input.module_dir.exists() {
        let _ = std::fs::remove_dir_all(&input.module_dir);
    }
    std::fs::create_dir_all(&input.module_dir).with_buck_error_context(|| {
        format!(
            "Failed to create {} cache directory for '{}' at {:?}",
            input.kind(),
            input.module_name,
            input.module_dir
        )
    })?;

    let cache = ModuleCache::new()?;
    let source_fetcher = crate::fetch::SourceFetcher::new(cache).await?;
    match &input.source {
        NonRegistryOverrideModuleSource::Git {
            remote,
            commit,
            shallow_since,
            patches,
            patch_strip,
        } => {
            tracing::info!(
                "Fetching git override for {} from {} at {}",
                input.module_name,
                remote,
                commit
            );
            let source_info = SourceInfo {
                source_type: Some("git_repository".to_owned()),
                url: None,
                urls: None,
                integrity: None,
                strip_prefix: None,
                overlay: crate::registry::RegistryFileMap::new(),
                patches: crate::registry::RegistryFileMap::new(),
                patch_strip: *patch_strip,
                remote: Some(remote.clone()),
                commit: Some(commit.clone()),
                shallow_since: shallow_since.clone(),
            };
            source_fetcher
                .fetch_git_direct(&source_info, &input.module_dir)
                .await?;
            crate::fetch::SourceFetcher::apply_local_override_patches_with_inputs(
                &input.module_dir,
                patches,
                *patch_strip,
                override_patch_inputs,
            )
            .with_buck_error_context(|| {
                format!(
                    "Failed to apply patches for git override '{}'",
                    input.module_name
                )
            })?;
        }
        NonRegistryOverrideModuleSource::Archive {
            urls,
            integrity,
            strip_prefix,
            patches,
            patch_strip,
        } => {
            tracing::info!(
                "Fetching archive override for {} from {:?}",
                input.module_name,
                urls
            );
            source_fetcher
                .fetch_archive_direct(
                    urls,
                    integrity.as_deref(),
                    strip_prefix.as_deref(),
                    &input.module_dir,
                )
                .await?;
            crate::fetch::SourceFetcher::apply_local_override_patches_with_inputs(
                &input.module_dir,
                patches,
                *patch_strip,
                override_patch_inputs,
            )
            .with_buck_error_context(|| {
                format!(
                    "Failed to apply patches for archive override '{}'",
                    input.module_name
                )
            })?;
        }
    }
    std::fs::write(&complete_marker, "").with_buck_error_context(|| {
        format!(
            "Failed to write {} completion marker for '{}' at {:?}",
            input.kind(),
            input.module_name,
            complete_marker
        )
    })?;
    Ok(())
}

/// The module name used by the canonical rules_python Bazel module. Matched
/// against `ParsedModuleFile::module.name` (the declared `module(name = ...)`
/// value), not against cell names.
const RULES_PYTHON_MODULE_NAME: &str = "rules_python";

/// Sentinel substring used to detect whether a user-registered toolchain label
/// already targets the bundled `@local_config_python` cell. Any label
/// containing this substring means we should not auto-inject duplicates.
const LOCAL_CONFIG_PYTHON_CELL: &str = "local_config_python";

/// Bundled toolchain labels auto-injected when `rules_python` is in the module
/// graph but the root module did not register a py3 toolchain.
///
/// Ordering matters: `host_toolchain` provides the default py3 runtime; the
/// launcher_maker stub satisfies rules_python 1.9+'s mandatory
/// launcher_maker_toolchain_type (only actually invoked on Windows, but
/// resolution must succeed on Linux/macOS too).
const BUNDLED_RULES_PYTHON_AUTO_INJECT_LABELS: &[&str] = &[
    "@local_config_python//:host_toolchain",
    "@local_config_python//:host_launcher_maker_toolchain",
];

/// Collect registered toolchain and execution platform outputs from parsed
/// modules using Bazel bzlmod dev-dependency visibility policy.
pub fn collect_bzlmod_registered_items(
    parsed_modules: &[(String, ParsedModuleFile)],
    root_module_name: &str,
    ignore_dev_dependency: bool,
) -> (Vec<RegisteredToolchain>, Vec<String>) {
    let mut all_toolchains = Vec::new();
    let mut all_exec_platforms = Vec::new();
    for (module_name, parsed_mod) in parsed_modules {
        let is_root = module_name == root_module_name
            || module_name == "_main"
            || parsed_mod.module.name == root_module_name;
        let repo_mapping = BzlmodRepoMapping::for_module(parsed_mod, root_module_name);
        for item in &parsed_mod.registered_toolchains {
            if item.dev_dependency && (!is_root || ignore_dev_dependency) {
                tracing::debug!(
                    "Skipping dev_dependency toolchain '{}' from module '{}'",
                    item.label,
                    module_name
                );
                continue;
            }
            let label = repo_mapping.canonicalize_label_to_storage_string(&item.label);
            all_toolchains.push(RegisteredToolchain {
                module: module_name.clone(),
                label,
                is_root,
            });
        }
        for item in &parsed_mod.registered_execution_platforms {
            if item.dev_dependency && (!is_root || ignore_dev_dependency) {
                tracing::debug!(
                    "Skipping dev_dependency execution platform '{}' from module '{}'",
                    item.label,
                    module_name
                );
                continue;
            }
            all_exec_platforms.push(repo_mapping.canonicalize_label_to_storage_string(&item.label));
        }
    }

    if module_depends_on_rules_python(parsed_modules)
        && !toolchains_include_bundled_python(&all_toolchains)
    {
        for label in BUNDLED_RULES_PYTHON_AUTO_INJECT_LABELS {
            all_toolchains.push(RegisteredToolchain {
                module: RULES_PYTHON_MODULE_NAME.to_owned(),
                label: (*label).to_owned(),
                is_root: true,
            });
        }
    }

    (all_toolchains, all_exec_platforms)
}

/// True iff `parsed_modules` contains the canonical rules_python module.
fn module_depends_on_rules_python(parsed_modules: &[(String, ParsedModuleFile)]) -> bool {
    parsed_modules
        .iter()
        .any(|(name, _)| name == RULES_PYTHON_MODULE_NAME)
}

/// True iff any toolchain label already references the bundled
/// `@local_config_python` cell (meaning the user has already wired up bundled
/// rules_python toolchains and we should skip auto-injection).
fn toolchains_include_bundled_python(toolchains: &[RegisteredToolchain]) -> bool {
    toolchains
        .iter()
        .any(|tc| tc.label.contains(LOCAL_CONFIG_PYTHON_CELL))
}

pub async fn resolve_graph_with_module_file_inputs(
    parsed: &ParsedModuleFile,
    workspace_root: &Path,
    options: &BzlmodResolutionOptions,
    local_override_inputs: &LocalOverrideModuleInputsValue,
    non_registry_override_inputs: &NonRegistryOverrideModuleInputsValue,
    override_patch_inputs: Arc<crate::OverridePatchInputs>,
    visible_lockfile: Option<&Lockfile>,
) -> slug_error::Result<ResolvedGraphWithModuleFileInputs> {
    let root_module_name = if parsed.module.name.is_empty() {
        "_main".to_owned()
    } else {
        parsed.module.name.clone()
    };
    let mut parsed_modules = vec![(root_module_name, parsed.clone())];
    if parsed.module.bazel_deps.is_empty() {
        return Ok(ResolvedGraphWithModuleFileInputs {
            graph: ResolvedGraph::default(),
            parsed_modules,
            non_root_module_file_inputs: Vec::new(),
        });
    }

    let active_deps: Vec<_> = parsed
        .module
        .bazel_deps
        .iter()
        .filter(|dep| !(options.ignore_dev_dependency && dep.dev_dependency))
        .collect();
    let local_override_paths: HashMap<_, _> =
        active_root_overrides(&parsed.module, options.ignore_dev_dependency)
            .into_iter()
            .filter_map(|override_| match override_ {
                Override::LocalPath(local) => Some((local.module_name, local.path)),
                _ => None,
            })
            .collect();
    let local_override_modules: HashMap<_, _> = local_override_inputs
        .parsed_modules
        .iter()
        .map(|(name, parsed)| (name.as_str(), parsed))
        .collect();
    let can_build_from_tracked_local_overrides = !active_deps.is_empty()
        && !local_override_inputs.has_bazel_deps
        && active_deps.iter().all(|dep| {
            local_override_paths.contains_key(&dep.name)
                && local_override_modules.contains_key(dep.name.as_str())
        });

    if can_build_from_tracked_local_overrides {
        let mut modules = fxhash::FxHashMap::default();
        let mut selected_versions = HashMap::new();
        let mut resolution_order = Vec::new();
        for dep in active_deps {
            let override_path = local_override_paths
                .get(&dep.name)
                .expect("local override path checked above");
            let parsed_override = local_override_modules
                .get(dep.name.as_str())
                .expect("local override module checked above");
            parsed_modules.push((dep.name.clone(), (*parsed_override).clone()));
            let version = parsed_override.module.version.to_string();
            selected_versions.insert(dep.name.clone(), version.clone());
            resolution_order.push(dep.name.clone());
            modules.insert(
                dep.name.clone(),
                ResolvedModuleInfo {
                    name: dep.name.clone(),
                    version,
                    compatibility_level: parsed_override.module.compatibility_level,
                    dependencies: HashMap::new(),
                    source: ModuleSource::LocalPath {
                        path: override_path.clone(),
                    },
                    source_path: Some(PathBuf::from(override_path)),
                },
            );
        }
        return Ok(ResolvedGraphWithModuleFileInputs {
            graph: ResolvedGraph {
                selected_versions,
                modules,
                resolution_order,
                registry_file_hashes: indexmap::IndexMap::new(),
                selected_yanked_versions: indexmap::IndexMap::new(),
            },
            parsed_modules,
            non_root_module_file_inputs: Vec::new(),
        });
    }

    let allowed_yanked_versions = parse_allowed_yanked_versions(
        options.allow_yanked_versions_env.as_deref(),
        &options.allow_yanked_versions_flags,
    )?;
    let cache = ModuleCache::new().with_buck_error_context(|| {
        format!(
            "Failed to initialize bzlmod module cache while computing clean graph for root module '{}'",
            parsed.module.name
        )
    })?;
    let mut resolver = MvsResolver::new(cache, override_patch_inputs)
        .await
        .with_buck_error_context(|| {
            format!(
                "Failed to create MVS resolver while computing clean graph for root module '{}'",
                parsed.module.name
            )
        })?;
    resolver.set_precomputed_local_override_modules(
        local_override_inputs.parsed_modules.iter().cloned(),
        local_override_inputs.missing_module_dirs.iter().cloned(),
    );
    let non_registry_module_dirs: HashMap<_, _> = non_registry_override_inputs
        .module_dirs
        .iter()
        .map(|(name, module_dir)| (name.as_str(), module_dir))
        .collect();
    let precomputed_non_registry_modules = non_registry_override_inputs
        .parsed_modules
        .iter()
        .map(|(name, parsed)| {
            let Some(module_dir) = non_registry_module_dirs.get(name.as_str()) else {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "DICE bzlmod non-registry override inputs for '{}' are missing the materialized module directory",
                    name
                ));
            };
            Ok((name.clone(), (*module_dir).clone(), parsed.clone()))
        })
        .collect::<slug_error::Result<Vec<_>>>()?;
    resolver.set_precomputed_non_registry_override_modules(precomputed_non_registry_modules);
    if let Some(lockfile) = visible_lockfile {
        resolver.set_yanked_version_policy(
            allowed_yanked_versions,
            options.lockfile_mode,
            lockfile.registry_file_hashes.clone(),
            lockfile.selected_yanked_versions.clone(),
        );
    } else {
        resolver.set_yanked_version_policy(
            allowed_yanked_versions,
            options.lockfile_mode,
            Default::default(),
            Default::default(),
        );
    }
    resolver.set_ignore_dev_dependency(options.ignore_dev_dependency);
    let mut graph = resolver
        .resolve(&parsed.module, workspace_root)
        .await
        .with_buck_error_context(|| {
            format!(
                "MVS resolution failed while computing clean graph for root module '{}' ({} direct dependencies)",
                parsed.module.name,
                parsed.module.bazel_deps.len()
            )
        })?;
    resolver
        .fetch_sources(&mut graph)
        .await
        .with_buck_error_context(|| {
            format!(
                "Failed to fetch selected module sources while computing clean graph for root module '{}'",
                parsed.module.name
            )
        })?;

    let non_registry_parsed_modules: HashMap<&str, &ParsedModuleFile> =
        non_registry_override_inputs
            .parsed_modules
            .iter()
            .map(|(name, parsed)| (name.as_str(), parsed))
            .collect();
    let non_registry_names: HashSet<&str> = non_registry_parsed_modules.keys().copied().collect();
    let mut non_root_module_file_inputs = Vec::new();
    for module_name in &graph.resolution_order {
        if non_registry_names.contains(module_name.as_str()) {
            continue;
        }
        let Some(module_info) = graph.modules.get(module_name) else {
            continue;
        };
        let Some(source_path) = module_info.source_path.as_ref() else {
            continue;
        };
        let module_dir = if source_path.is_absolute() {
            source_path.clone()
        } else {
            workspace_root.join(source_path)
        };
        let module_bazel_path = module_dir.join("MODULE.bazel");
        non_root_module_file_inputs.push(NonRootModuleFileInput {
            module_key: module_name.clone(),
            module_bazel_path,
        });
    }

    Ok(ResolvedGraphWithModuleFileInputs {
        graph,
        parsed_modules,
        non_root_module_file_inputs,
    })
}

pub fn append_resolved_non_root_modules(
    parsed_modules: &mut Vec<(String, ParsedModuleFile)>,
    graph: &ResolvedGraph,
    non_registry_override_inputs: &NonRegistryOverrideModuleInputsValue,
    dice_parsed_modules: &[(String, ParsedModuleFile)],
) {
    let non_registry_parsed_modules: HashMap<&str, &ParsedModuleFile> =
        non_registry_override_inputs
            .parsed_modules
            .iter()
            .map(|(name, parsed)| (name.as_str(), parsed))
            .collect();
    let dice_parsed: HashMap<&str, &ParsedModuleFile> = dice_parsed_modules
        .iter()
        .map(|(name, parsed)| (name.as_str(), parsed))
        .collect();
    for module_name in &graph.resolution_order {
        if parsed_modules
            .iter()
            .any(|(name, _)| name.as_str() == module_name.as_str())
        {
            continue;
        }
        if let Some(parsed) = non_registry_parsed_modules.get(module_name.as_str()) {
            parsed_modules.push((module_name.clone(), (*parsed).clone()));
        } else if let Some(parsed) = dice_parsed.get(module_name.as_str()) {
            parsed_modules.push((module_name.clone(), (*parsed).clone()));
        }
    }
}

pub fn resolved_graph_projection_values(
    workspace_id: WorkspaceId,
    root_module: &ParsedModuleFile,
    parsed_modules: &[(String, ParsedModuleFile)],
    graph: &ResolvedGraph,
    ignore_dev_dependency: bool,
) -> BzlmodResolvedGraphProjectionValues {
    let root_module_name = parsed_modules
        .first()
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| "_main".to_owned());
    let mut version_map = HashMap::new();
    version_map.insert(
        root_module.module.name.clone(),
        root_module.module.version.to_string(),
    );
    for (name, info) in &graph.modules {
        version_map.insert(name.clone(), info.version.clone());
    }
    let module_versions = BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
        workspace_id.clone(),
        root_module_name.clone(),
        Arc::new(version_map),
    );
    let resolution_facts = BzlmodResolutionFactsValue::for_workspace(
        workspace_id.clone(),
        graph.registry_file_hashes.clone(),
        graph.selected_yanked_versions.clone(),
    );
    let (all_toolchains, all_exec_platforms) =
        collect_bzlmod_registered_items(parsed_modules, &root_module_name, ignore_dev_dependency);
    let registered_toolchains =
        RegisteredToolchainsDataValue::for_workspace(workspace_id.clone(), all_toolchains);
    let registered_execution_platforms = RegisteredExecutionPlatformsDataValue::for_workspace(
        workspace_id.clone(),
        all_exec_platforms,
    );
    let mut module_extensions: HashMap<String, Vec<crate::ExtensionUsage>> = HashMap::new();
    for (module_name, parsed_mod) in parsed_modules {
        if !parsed_mod.extension_usages.is_empty() {
            module_extensions.insert(module_name.clone(), parsed_mod.extension_usages.clone());
        }
    }
    let extension_aggregations =
        BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
            workspace_id,
            root_module_name.clone(),
            Arc::new(crate::aggregate_extensions_with_policy(
                &module_extensions,
                Some(root_module_name.as_str()),
                ignore_dev_dependency,
            )),
        );

    BzlmodResolvedGraphProjectionValues {
        module_versions,
        resolution_facts,
        registered_toolchains,
        registered_execution_platforms,
        extension_aggregations,
    }
}

pub fn clean_resolved_graph_outputs_value(
    workspace_id: WorkspaceId,
    cell_graph_resolution_digest: Arc<str>,
    root_module: &ParsedModuleFile,
    parsed_modules: &[(String, ParsedModuleFile)],
    graph: ResolvedGraph,
    ignore_dev_dependency: bool,
    cell_graph: BzlmodCellGraphValue,
) -> BzlmodResolvedGraphOutputsValue {
    let projections = resolved_graph_projection_values(
        workspace_id.clone(),
        root_module,
        parsed_modules,
        &graph,
        ignore_dev_dependency,
    );
    let clean_cell_names = resolved_graph_cell_names(&graph);
    let clean_cell_name_refs: Vec<_> = clean_cell_names.iter().map(String::as_str).collect();
    let (repo_mapping_snapshot, repo_mapping_overrides) = graph_owned_repo_mapping_state(
        parsed_modules,
        &projections.module_versions.root_module_name,
        ignore_dev_dependency,
        &clean_cell_name_refs,
        Some(&graph),
    );
    let repo_mappings = BzlmodRepoMappingsDataValue::for_workspace(
        workspace_id,
        Arc::new(repo_mapping_snapshot),
        Arc::new(repo_mapping_overrides),
    )
    .with_declared_aliases(
        cell_graph.root_aliases.dupe(),
        cell_graph.scoped_aliases.dupe(),
        cell_graph.dynamic_aliases.dupe(),
    );
    let graph_digest = bzlmod_resolved_graph_digest(&graph);
    let declared_extension_cells = cell_graph.extension_cells.dupe();

    BzlmodResolvedGraphOutputsValue {
        graph: Arc::new(graph),
        graph_digest: Arc::from(graph_digest.as_str()),
        cell_graph_resolution_digest: cell_graph_resolution_digest.clone(),
        module_versions: projections
            .module_versions
            .with_resolution_digest(cell_graph_resolution_digest.clone()),
        resolution_facts: projections
            .resolution_facts
            .with_resolution_digest(cell_graph_resolution_digest.clone()),
        registered_toolchains: projections
            .registered_toolchains
            .with_resolution_digest(cell_graph_resolution_digest.clone()),
        registered_execution_platforms: projections
            .registered_execution_platforms
            .with_resolution_digest(cell_graph_resolution_digest.clone()),
        extension_aggregations: projections
            .extension_aggregations
            .with_declared_extension_cells(declared_extension_cells)
            .with_resolution_digest(cell_graph_resolution_digest.clone()),
        repo_mappings: repo_mappings.with_resolution_digest(cell_graph_resolution_digest),
        cell_graph,
    }
}

static LAST_RECORDED_BZLMOD_RESOLUTION_DIGEST: OnceLock<Mutex<HashMap<String, String>>> =
    OnceLock::new();

fn record_clean_bzlmod_resolution_compute_if_changed(
    key: &BzlmodResolvedModuleGraphKey,
    resolution_key: &BzlmodResolutionKey,
    inputs: &BzlmodResolvedGraphSourceInputsValue,
) {
    let cache_key = format!(
        "{}:{}",
        resolution_key.workspace_id.stable_hash(),
        resolution_key.command_policy_digest
    );
    let input_digest = inputs.identity_digest_with_key(key);
    let mut last = LAST_RECORDED_BZLMOD_RESOLUTION_DIGEST
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if last.get(&cache_key).map(String::as_str) == Some(input_digest.as_str()) {
        return;
    }
    last.insert(cache_key, input_digest);
    record_bzlmod_event(
        BzlmodEventKind::BzlmodResolutionCompute,
        inputs.root_module_file.path.display().to_string(),
    );
}

async fn compute_bzlmod_resolved_module_graph(
    key: &BzlmodResolvedModuleGraphKey,
    dice_ctx: &mut DiceComputations<'_>,
) -> slug_error::Result<Arc<BzlmodResolvedModuleGraphValue>> {
    let command_policy = dice_ctx
        .compute(&key.options.command_policy_key(key.workspace_id.clone()))
        .await?
        .buck_error_context("Computing bzlmod command policy")?;
    let resolution_key = BzlmodResolutionKey {
        workspace_id: key.workspace_id.clone(),
        command_policy_digest: command_policy.digest.clone(),
    };
    let io = *BZLMOD_CLEAN_GRAPH_IO_IMPL.get()?;
    let inputs = io.compute_source_inputs(key, dice_ctx).await?;
    let Some(parsed) = inputs.root_module_file.parsed.clone() else {
        return Ok(Arc::new(BzlmodResolvedModuleGraphValue {
            lockfile_inputs: inputs.lockfile_inputs,
            outputs: Arc::new(None),
        }));
    };
    if key.validate_root_extension_repo_directives && !key.options.ignore_dev_dependency {
        validate_parsed_root_extension_repo_directives(&parsed)?;
    }

    let visible_lockfile = if key.options.lockfile_mode == LockfileMode::Off {
        None
    } else if let Some(visible_lockfile) = inputs.lockfile_inputs.visible_lockfile.as_ref() {
        visible_lockfile.lockfile.clone()
    } else {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Input,
            "clean DICE bzlmod resolved graph requires tracked visible lockfile input"
        ));
    };
    let _hidden_lockfile = if key.options.lockfile_mode == LockfileMode::Off {
        None
    } else if let Some(hidden_lockfile) = inputs.lockfile_inputs.hidden_lockfile.as_ref() {
        hidden_lockfile.lockfile.clone()
    } else if key.options.hidden_lockfile_path.is_some() {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Input,
            "clean DICE bzlmod resolved graph requires tracked hidden lockfile input"
        ));
    } else {
        None
    };

    let workspace_root = key.workspace_id.canonical_project_root.as_ref();
    let graph_inputs = resolve_graph_with_module_file_inputs(
        &parsed,
        workspace_root,
        &key.options,
        inputs.local_override_inputs.as_ref(),
        inputs.non_registry_override_inputs.as_ref(),
        inputs.override_patch_inputs.clone(),
        visible_lockfile.as_deref(),
    )
    .await?;
    let graph = graph_inputs.graph;
    let mut parsed_modules = graph_inputs.parsed_modules;
    if !graph_inputs.non_root_module_file_inputs.is_empty() {
        let non_root_value = io
            .compute_non_root_module_files(
                key,
                dice_ctx,
                graph_inputs.non_root_module_file_inputs,
                &parsed.module.name,
            )
            .await
            .with_buck_error_context(|| {
                format!(
                    "Failed to parse non-root MODULE.bazel files via DICE while computing clean graph for root module '{}'",
                    parsed.module.name
                )
            })?;
        append_resolved_non_root_modules(
            &mut parsed_modules,
            &graph,
            inputs.non_registry_override_inputs.as_ref(),
            &non_root_value.parsed_modules,
        );
    } else {
        append_resolved_non_root_modules(
            &mut parsed_modules,
            &graph,
            inputs.non_registry_override_inputs.as_ref(),
            &[],
        );
    }

    let mut builder = BzlmodCleanCellGraphBuilder::new(
        key.workspace_id.clone(),
        &key.options,
        &parsed,
        &graph,
        &parsed_modules,
    )?;
    builder
        .resolve_use_repo_rule_local_bits(dice_ctx, &parsed_modules)
        .await?;
    builder.install_precomputed_extension_mapping_rows();
    let cell_graph = builder.finish()?;
    let cell_graph_resolution_digest = Arc::from(inputs.identity_digest_with_key(key).as_str());
    let outputs = clean_resolved_graph_outputs_value(
        key.workspace_id.clone(),
        cell_graph_resolution_digest,
        &parsed,
        &parsed_modules,
        graph,
        key.options.ignore_dev_dependency,
        cell_graph,
    );
    record_clean_bzlmod_resolution_compute_if_changed(key, &resolution_key, &inputs);
    Ok(Arc::new(BzlmodResolvedModuleGraphValue {
        lockfile_inputs: inputs.lockfile_inputs,
        outputs: Arc::new(Some(outputs)),
    }))
}

pub fn repo_mapping_snapshot_for_modules(
    parsed_modules: &[(String, ParsedModuleFile)],
    root_module_name: &str,
) -> crate::RepoMappingSnapshot {
    repo_mapping_snapshot_for_modules_with_policy(parsed_modules, root_module_name, false)
}

pub fn repo_mapping_snapshot_for_modules_with_policy(
    parsed_modules: &[(String, ParsedModuleFile)],
    root_module_name: &str,
    ignore_dev_dependency: bool,
) -> crate::RepoMappingSnapshot {
    let mut snapshot = crate::RepoMappingSnapshot::new();
    for (module_name, parsed_mod) in parsed_modules {
        let mapping = BzlmodRepoMapping::for_module_with_policy(
            parsed_mod,
            root_module_name,
            ignore_dev_dependency,
        )
        .entries_as_strings();
        if module_name == root_module_name {
            snapshot.insert(String::new(), mapping.clone());
        }
        snapshot.insert(module_name.clone(), mapping);
    }
    snapshot
}

pub fn graph_owned_repo_mapping_state(
    parsed_modules: &[(String, ParsedModuleFile)],
    root_module_name: &str,
    ignore_dev_dependency: bool,
    cell_names: &[&str],
    resolved_graph: Option<&ResolvedGraph>,
) -> (crate::RepoMappingSnapshot, crate::RepoMappingOverrides) {
    let mut repo_mappings = repo_mapping_snapshot_for_modules_with_policy(
        parsed_modules,
        root_module_name,
        ignore_dev_dependency,
    );
    let mut repo_mapping_overrides =
        repo_mapping_overrides_for_root(parsed_modules, root_module_name, ignore_dev_dependency);
    canonicalize_repo_mapping_targets(
        &mut repo_mappings,
        &mut repo_mapping_overrides,
        cell_names,
        resolved_graph,
    );
    (repo_mappings, repo_mapping_overrides)
}

pub fn canonicalize_repo_mapping_snapshot_targets(
    snapshot: &mut crate::RepoMappingSnapshot,
    cell_names: &[&str],
    resolved_graph: Option<&ResolvedGraph>,
) {
    for mapping in snapshot.values_mut() {
        for target_name in mapping.values_mut() {
            *target_name =
                canonical_repo_mapping_target_name(None, cell_names, resolved_graph, target_name);
        }
    }

    let root_repo_mapping = snapshot.get("").cloned();
    for mapping in snapshot.values_mut() {
        for target_name in mapping.values_mut() {
            *target_name = canonical_repo_mapping_target_name(
                root_repo_mapping.as_ref(),
                cell_names,
                resolved_graph,
                target_name,
            );
        }
    }
}

fn canonicalize_repo_mapping_targets(
    repo_mappings: &mut crate::RepoMappingSnapshot,
    repo_mapping_overrides: &mut crate::RepoMappingOverrides,
    cell_names: &[&str],
    resolved_graph: Option<&ResolvedGraph>,
) {
    canonicalize_repo_mapping_snapshot_targets(repo_mappings, cell_names, resolved_graph);
    canonicalize_repo_mapping_overrides_targets(
        repo_mapping_overrides,
        repo_mappings,
        cell_names,
        resolved_graph,
    );
}

pub fn canonicalize_repo_mapping_overrides_targets(
    overrides: &mut crate::RepoMappingOverrides,
    repo_mappings: &crate::RepoMappingSnapshot,
    cell_names: &[&str],
    resolved_graph: Option<&ResolvedGraph>,
) {
    let root_repo_mapping = repo_mappings.get("");
    for overrides in overrides.values_mut() {
        for target_name in overrides.values_mut() {
            *target_name = canonical_repo_mapping_target_name(
                root_repo_mapping,
                cell_names,
                resolved_graph,
                target_name,
            );
        }
    }
}

fn canonical_repo_mapping_target_name(
    root_repo_mapping: Option<&BTreeMap<String, String>>,
    cell_names: &[&str],
    resolved_graph: Option<&ResolvedGraph>,
    target_name: &str,
) -> String {
    let mut current = target_name.to_owned();
    let mut seen = BTreeSet::new();

    loop {
        if !seen.insert(current.clone()) {
            return current;
        }
        let next = root_repo_mapping
            .and_then(|mapping| mapping.get(&current))
            .cloned()
            .or_else(|| {
                resolved_graph
                    .and_then(|graph| {
                        selected_bzlmod_cell_name_for_dep(cell_names, &current, graph)
                    })
                    .map(str::to_owned)
            });
        let Some(next) = next else {
            return current;
        };
        if next == current {
            return current;
        }
        current = next;
    }
}

fn repo_mapping_overrides_for_root(
    parsed_modules: &[(String, ParsedModuleFile)],
    root_module_name: &str,
    ignore_dev_dependency: bool,
) -> crate::RepoMappingOverrides {
    let mut overrides = crate::RepoMappingOverrides::new();
    if ignore_dev_dependency {
        return overrides;
    }
    for (cell_name, parsed_mod) in parsed_modules {
        let module_name = if parsed_mod.module.name.is_empty() {
            root_module_name
        } else {
            &parsed_mod.module.name
        };
        let is_root = cell_name == root_module_name
            || cell_name == "_main"
            || parsed_mod.module.name == root_module_name;
        if !is_root {
            continue;
        }

        for usage in &parsed_mod.extension_usages {
            if usage.repo_overrides.is_empty() && usage.injected_repos.is_empty() {
                continue;
            }
            let ext_id = crate::canonical_extension_id_for_usage(usage, module_name);
            let entry = overrides.entry(ext_id).or_default();
            for (generated_name, replacement_repo) in &usage.repo_overrides {
                entry.insert(generated_name.clone(), replacement_repo.clone());
            }
            for (injected_name, source_repo) in &usage.injected_repos {
                entry.insert(injected_name.clone(), source_repo.clone());
            }
        }
    }
    overrides
}

pub fn selected_bzlmod_cell_name_for_dep<'a>(
    cell_names: &[&'a str],
    dep_name: &str,
    resolved_graph: &ResolvedGraph,
) -> Option<&'a str> {
    if let Some(name) = cell_names.iter().copied().find(|name| *name == dep_name) {
        return Some(name);
    }

    let selected_version = resolved_graph
        .modules
        .get(dep_name)
        .map(|module| module.version.as_str())
        .or_else(|| {
            resolved_graph
                .selected_versions
                .get(dep_name)
                .map(String::as_str)
        })?;
    let canonical_name = bazel_canonical_module_repo_name(dep_name, selected_version);
    if let Some(name) = cell_names
        .iter()
        .copied()
        .find(|name| *name == canonical_name)
    {
        return Some(name);
    }

    let versioned_name = format!("{}+{}", dep_name, selected_version);
    if let Some(name) = cell_names
        .iter()
        .copied()
        .find(|name| *name == versioned_name)
    {
        return Some(name);
    }

    None
}

pub fn module_file_inputs_digest(inputs: &[ModuleFileInputDigest]) -> String {
    let mut hasher = Sha256::new();
    for input in inputs {
        hasher.update(input.path.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(input.digest.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleSourceKey {
    pub workspace_id: WorkspaceId,
    pub command_policy_digest: Arc<str>,
    pub module_key: ModuleKey,
    pub source: ModuleSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleFileKey {
    pub workspace_id: WorkspaceId,
    pub source_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct LocalOverrideSourceKey {
    pub workspace_id: WorkspaceId,
    pub declaring_module: ModuleKey,
    pub override_literal_digest: Arc<str>,
    pub resolved_abs_path: Arc<PathBuf>,
    pub module_file_digest: Arc<str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Allocative)]
pub enum LockfileContentKind {
    Workspace,
    Hidden,
}

/// DICE-owned lockfile read result.
#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct LockfileContentValue {
    pub path: Arc<PathBuf>,
    pub digest: Option<String>,
    pub tracked_by_dice: bool,
    #[allocative(skip)]
    pub lockfile: Option<Arc<Lockfile>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodLockfileInputsValue {
    pub hidden_lockfile_path: Option<PathBuf>,
    pub visible_lockfile_digest: Option<String>,
    pub hidden_lockfile_digest: Option<String>,
    pub visible_lockfile: Option<Arc<LockfileContentValue>>,
    pub hidden_lockfile: Option<Arc<LockfileContentValue>>,
    pub lockfile_mode: crate::LockfileMode,
}

impl BzlmodLockfileInputsValue {
    pub fn from_values(
        hidden_lockfile_path: Option<PathBuf>,
        visible_lockfile: Option<Arc<LockfileContentValue>>,
        hidden_lockfile: Option<Arc<LockfileContentValue>>,
        lockfile_mode: crate::LockfileMode,
    ) -> Self {
        Self {
            hidden_lockfile_path,
            visible_lockfile_digest: lockfile_content_digest(&visible_lockfile),
            hidden_lockfile_digest: lockfile_content_digest(&hidden_lockfile),
            visible_lockfile,
            hidden_lockfile,
            lockfile_mode,
        }
    }

    pub fn identity_eq(&self, other: &Self) -> bool {
        self.hidden_lockfile_path == other.hidden_lockfile_path
            && self.visible_lockfile_digest == other.visible_lockfile_digest
            && self.hidden_lockfile_digest == other.hidden_lockfile_digest
            && self.lockfile_mode == other.lockfile_mode
            && lockfile_content_identity_eq(&self.visible_lockfile, &other.visible_lockfile)
            && lockfile_content_identity_eq(&self.hidden_lockfile, &other.hidden_lockfile)
    }

    pub fn hash_identity<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hidden_lockfile_path.hash(state);
        self.visible_lockfile_digest.hash(state);
        self.hidden_lockfile_digest.hash(state);
        self.lockfile_mode.hash(state);
        hash_lockfile_content_identity(&self.visible_lockfile, state);
        hash_lockfile_content_identity(&self.hidden_lockfile, state);
    }

    pub fn has_untracked_inputs(&self) -> bool {
        self.visible_lockfile
            .as_ref()
            .is_some_and(|value| !value.tracked_by_dice)
            || self
                .hidden_lockfile
                .as_ref()
                .is_some_and(|value| !value.tracked_by_dice)
    }
}

impl Default for BzlmodLockfileInputsValue {
    fn default() -> Self {
        Self::from_values(None, None, None, crate::LockfileMode::Update)
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "BzlmodCleanLockfileInputsKey({}, {:?})",
    workspace_id.stable_hash(),
    lockfile_mode
)]
pub struct BzlmodCleanLockfileInputsKey {
    pub workspace_id: WorkspaceId,
    pub lockfile_mode: crate::LockfileMode,
    pub hidden_lockfile_path: Option<PathBuf>,
    pub root_module_present: bool,
}

#[async_trait]
impl Key for BzlmodCleanLockfileInputsKey {
    type Value = slug_error::Result<Arc<BzlmodLockfileInputsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if !self.root_module_present || self.lockfile_mode == crate::LockfileMode::Off {
            return Ok(Arc::new(BzlmodLockfileInputsValue::from_values(
                self.hidden_lockfile_path.clone(),
                None,
                None,
                self.lockfile_mode,
            )));
        }

        let io = *BZLMOD_CLEAN_GRAPH_IO_IMPL.get()?;
        let visible_path = lockfile_path(self.workspace_id.canonical_project_root.as_ref());
        let visible_lockfile = io
            .compute_lockfile_content(
                &self.workspace_id,
                LockfileContentKind::Workspace,
                Arc::new(visible_path),
                ctx,
            )
            .await?;
        let hidden_lockfile = match &self.hidden_lockfile_path {
            Some(path) => Some(
                io.compute_lockfile_content(
                    &self.workspace_id,
                    LockfileContentKind::Hidden,
                    Arc::new(path.clone()),
                    ctx,
                )
                .await?,
            ),
            None => None,
        };

        Ok(Arc::new(BzlmodLockfileInputsValue::from_values(
            self.hidden_lockfile_path.clone(),
            Some(visible_lockfile),
            hidden_lockfile,
            self.lockfile_mode,
        )))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.identity_eq(y),
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        match x {
            Ok(value) => !value.has_untracked_inputs(),
            Err(_) => false,
        }
    }
}

fn lockfile_content_digest(value: &Option<Arc<LockfileContentValue>>) -> Option<String> {
    value.as_ref().and_then(|value| value.digest.clone())
}

fn lockfile_content_identity_eq(
    left: &Option<Arc<LockfileContentValue>>,
    right: &Option<Arc<LockfileContentValue>>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => match (&left.digest, &right.digest) {
            (Some(_), Some(_)) => left.path == right.path && left.digest == right.digest,
            (None, None) => true,
            _ => false,
        },
        (None, None) => true,
        _ => false,
    }
}

fn hash_lockfile_content_identity<H: std::hash::Hasher>(
    value: &Option<Arc<LockfileContentValue>>,
    state: &mut H,
) {
    match value {
        Some(value) => {
            true.hash(state);
            if value.digest.is_some() {
                value.path.hash(state);
            }
            value.digest.hash(state);
        }
        None => false.hash(state),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct LockfileExtensionEntryKey {
    pub workspace_id: WorkspaceId,
    pub extension_instance_id: ModuleExtensionId,
    pub lockfile_digest: Arc<str>,
    pub lockfile_mode: Arc<str>,
    pub eval_factors_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct BzlmodResolutionKey {
    pub workspace_id: WorkspaceId,
    pub command_policy_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ResolvedModuleIdentity {
    pub workspace_id: WorkspaceId,
    pub module_key: ModuleKey,
    pub canonical_repo_name: Arc<str>,
    pub apparent_repo_name: Arc<str>,
    pub module_name: Arc<str>,
    pub version: Arc<str>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "BzlmodCellGraphKey({}, {})",
    workspace_id.stable_hash(),
    resolution_digest
)]
pub struct BzlmodCellGraphKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

impl BzlmodCellGraphKey {
    pub fn for_workspace_id_and_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id_and_resolution_digest(
            WorkspaceId::for_project_root(project_root),
            Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodCellGraphModuleSetup {
    pub module_name: String,
    pub version: String,
    pub registry_url: String,
    pub source_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodCellGraphCell {
    pub name: String,
    pub path: String,
    pub module_setup: Option<BzlmodCellGraphModuleSetup>,
    pub bundled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodCellGraphExtensionCell {
    pub canonical_name: String,
    pub internal_name: String,
    pub path: String,
    pub extension_id: String,
    pub spec_hash: String,
    pub repo_spec_json: String,
    pub repo_env_json: String,
    pub extension_usages_digest: String,
    pub extension_replay_inputs_identity_digest: String,
    pub extension_repo_mappings_digest: String,
    pub extension_repo_mapping_overrides_digest: String,
    pub extension_bzl_transitive_digest: String,
    pub extension_recorded_inputs_json: String,
    pub materialized: bool,
    pub lazy: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodCellGraphAlias {
    pub apparent_name: String,
    pub target_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodCellGraphModuleSymlink {
    pub entry_name: String,
    pub source_path: Arc<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodCellGraphScopedAlias {
    pub owner_module: String,
    pub apparent_name: String,
    pub target_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodCellGraphDynamicAlias {
    pub apparent_name: String,
    pub canonical_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodCellGraphValue {
    pub workspace_id: WorkspaceId,
    pub root_module_name: String,
    pub cells: Arc<Vec<BzlmodCellGraphCell>>,
    pub extension_cells: Arc<Vec<BzlmodCellGraphExtensionCell>>,
    pub root_aliases: Arc<Vec<BzlmodCellGraphAlias>>,
    pub module_symlinks: Arc<Vec<BzlmodCellGraphModuleSymlink>>,
    pub scoped_aliases: Arc<Vec<BzlmodCellGraphScopedAlias>>,
    pub dynamic_aliases: Arc<Vec<BzlmodCellGraphDynamicAlias>>,
}

impl BzlmodCellGraphValue {
    pub fn empty_for_workspace(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            root_module_name: String::new(),
            cells: Arc::new(Vec::new()),
            extension_cells: Arc::new(Vec::new()),
            root_aliases: Arc::new(Vec::new()),
            module_symlinks: Arc::new(Vec::new()),
            scoped_aliases: Arc::new(Vec::new()),
            dynamic_aliases: Arc::new(Vec::new()),
        }
    }
}

pub struct BzlmodCleanCellGraphBuilder {
    workspace_id: WorkspaceId,
    root_module_name: String,
    cells: Vec<BzlmodCellGraphCell>,
    root_aliases: Vec<BzlmodCellGraphAlias>,
    module_symlinks: Vec<BzlmodCellGraphModuleSymlink>,
    scoped_aliases: Vec<BzlmodCellGraphScopedAlias>,
    dynamic_aliases: Vec<BzlmodCellGraphDynamicAlias>,
    selected_module_versions: HashMap<String, String>,
    aggregated_extensions: HashMap<String, AggregatedExtension>,
    repo_mappings: crate::RepoMappingSnapshot,
    repo_mapping_overrides: crate::RepoMappingOverrides,
    pre_computed_cells: Vec<crate::pending_repo_cells::PendingRepoCell>,
    pre_computed_aliases: Vec<crate::pending_repo_cells::RepoAlias>,
    extension_mapping_cells: Vec<crate::pending_repo_cells::PendingRepoCell>,
    repo_env_json: String,
}

impl BzlmodCleanCellGraphBuilder {
    pub fn new(
        workspace_id: WorkspaceId,
        options: &BzlmodResolutionOptions,
        parsed: &ParsedModuleFile,
        graph: &ResolvedGraph,
        parsed_modules: &[(String, ParsedModuleFile)],
    ) -> slug_error::Result<Self> {
        let mut cells = Vec::new();
        let mut root_aliases = Vec::new();
        let mut module_symlinks = Vec::new();
        let workspace_root = workspace_id.canonical_project_root.as_ref();
        let root_module_name = if parsed.module.name.is_empty() {
            "_main".to_owned()
        } else {
            parsed.module.name.clone()
        };
        let selected_module_versions = selected_module_versions_from_graph(graph);

        if !parsed.module.bazel_deps.is_empty() {
            let mut sorted_modules: Vec<_> = graph.modules.iter().collect();
            sorted_modules.sort_by(|a, b| a.0.cmp(b.0));
            for (module_name, module_info) in sorted_modules {
                if module_name == &parsed.module.name || module_name == &root_module_name {
                    continue;
                }

                let canonical_repo =
                    bazel_canonical_module_repo_name(module_name, &module_info.version);
                match &module_info.source {
                    ModuleSource::Registry { url } => {
                        if let Some(source_path) = &module_info.source_path {
                            module_symlinks.push(BzlmodCellGraphModuleSymlink {
                                entry_name: canonical_repo.clone(),
                                source_path: Arc::new(source_path.clone()),
                            });
                        }
                        let source_path = module_info
                            .source_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        cells.push(BzlmodCellGraphCell {
                            name: canonical_repo.clone(),
                            path: format!("bazel-external/{canonical_repo}"),
                            module_setup: Some(BzlmodCellGraphModuleSetup {
                                module_name: module_name.clone(),
                                version: module_info.version.clone(),
                                registry_url: url.clone(),
                                source_path,
                            }),
                            bundled: false,
                        });
                    }
                    ModuleSource::LocalPath { path } => {
                        let (cell_path, symlink) = local_override_cell_path_and_symlink(
                            workspace_root,
                            module_name,
                            &module_info.version,
                            path,
                        );
                        if let Some(symlink) = symlink {
                            module_symlinks.push(symlink);
                        }
                        cells.push(BzlmodCellGraphCell {
                            name: canonical_repo,
                            path: cell_path,
                            module_setup: None,
                            bundled: false,
                        });
                    }
                    ModuleSource::Git {
                        remote, commit: _, ..
                    } => {
                        if let Some(source_path) = &module_info.source_path {
                            module_symlinks.push(BzlmodCellGraphModuleSymlink {
                                entry_name: canonical_repo.clone(),
                                source_path: Arc::new(source_path.clone()),
                            });
                        }
                        let source_path = module_info
                            .source_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        cells.push(BzlmodCellGraphCell {
                            name: canonical_repo.clone(),
                            path: format!("bazel-external/{canonical_repo}"),
                            module_setup: Some(BzlmodCellGraphModuleSetup {
                                module_name: module_name.clone(),
                                version: module_info.version.clone(),
                                registry_url: format!("git+{remote}"),
                                source_path,
                            }),
                            bundled: false,
                        });
                    }
                    ModuleSource::Archive { urls, .. } => {
                        if let Some(source_path) = &module_info.source_path {
                            module_symlinks.push(BzlmodCellGraphModuleSymlink {
                                entry_name: canonical_repo.clone(),
                                source_path: Arc::new(source_path.clone()),
                            });
                        }
                        let source_path = module_info
                            .source_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let url = urls
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "archive".to_owned());
                        cells.push(BzlmodCellGraphCell {
                            name: canonical_repo.clone(),
                            path: format!("bazel-external/{canonical_repo}"),
                            module_setup: Some(BzlmodCellGraphModuleSetup {
                                module_name: module_name.clone(),
                                version: module_info.version.clone(),
                                registry_url: url,
                                source_path,
                            }),
                            bundled: false,
                        });
                    }
                }
            }

            if let Some(repo_name) = &parsed.module.repo_name {
                if repo_name != &parsed.module.name {
                    root_aliases.push(BzlmodCellGraphAlias {
                        apparent_name: repo_name.clone(),
                        target_name: parsed.module.name.clone(),
                    });
                }
            }

            for dep in &parsed.module.bazel_deps {
                let apparent_name = dep.apparent_name();
                if let Some(target_name) = selected_bzlmod_cell_name_for_dep_in_graph_cells(
                    &cells,
                    &dep.name,
                    &selected_module_versions,
                ) {
                    root_aliases.push(BzlmodCellGraphAlias {
                        apparent_name: apparent_name.to_owned(),
                        target_name: target_name.to_owned(),
                    });
                }
            }

            for module_name in graph.modules.keys() {
                if module_name == &parsed.module.name || module_name == &root_module_name {
                    continue;
                }
                if root_aliases
                    .iter()
                    .any(|alias| alias.apparent_name == *module_name)
                {
                    continue;
                }
                if let Some(target_name) = selected_bzlmod_cell_name_for_dep_in_graph_cells(
                    &cells,
                    module_name,
                    &selected_module_versions,
                ) {
                    if module_name == target_name {
                        continue;
                    }
                    root_aliases.push(BzlmodCellGraphAlias {
                        apparent_name: module_name.clone(),
                        target_name: target_name.to_owned(),
                    });
                }
            }
        }

        let mut scoped_aliases = Vec::new();
        for (module_name, parsed_mod) in parsed_modules {
            for dep in &parsed_mod.module.bazel_deps {
                let apparent_name = dep.apparent_name();
                let Some(target_name) = selected_bzlmod_cell_name_for_dep_in_graph_cells(
                    &cells,
                    &dep.name,
                    &selected_module_versions,
                ) else {
                    continue;
                };
                if apparent_name == target_name {
                    continue;
                }
                if module_name != &root_module_name {
                    scoped_aliases.push(BzlmodCellGraphScopedAlias {
                        owner_module: module_name.clone(),
                        apparent_name: apparent_name.to_owned(),
                        target_name: target_name.to_owned(),
                    });
                    continue;
                }
                if root_aliases
                    .iter()
                    .any(|alias| alias.apparent_name == apparent_name)
                {
                    continue;
                }
                root_aliases.push(BzlmodCellGraphAlias {
                    apparent_name: apparent_name.to_owned(),
                    target_name: target_name.to_owned(),
                });
            }
        }

        let mut module_extensions: HashMap<String, Vec<crate::ExtensionUsage>> = HashMap::new();
        for (module_name, parsed_mod) in parsed_modules {
            if !parsed_mod.extension_usages.is_empty() {
                module_extensions.insert(module_name.clone(), parsed_mod.extension_usages.clone());
            }
        }
        let aggregated_extensions = aggregate_extensions_with_policy(
            &module_extensions,
            Some(&root_module_name),
            options.ignore_dev_dependency,
        );
        let cell_names = cell_name_strs_from_graph_cells(&cells);
        let (repo_mappings, repo_mapping_overrides) = graph_owned_repo_mapping_state(
            parsed_modules,
            &root_module_name,
            options.ignore_dev_dependency,
            &cell_names,
            Some(graph),
        );
        let (pre_computed_cells, pre_computed_aliases) =
            crate::pending_repo_cells::pre_compute_extension_repo_cells(
                parsed_modules,
                &root_module_name,
                options.ignore_dev_dependency,
            )?;
        let repo_env_json = serde_json::to_string(&options.repo_env).map_err(|e| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "Failed to serialize bzlmod repo_env for clean cell graph: {}",
                e
            )
        })?;

        Ok(Self {
            workspace_id,
            root_module_name,
            cells,
            root_aliases,
            module_symlinks,
            scoped_aliases,
            dynamic_aliases: Vec::new(),
            selected_module_versions,
            aggregated_extensions,
            repo_mappings,
            repo_mapping_overrides,
            pre_computed_cells,
            pre_computed_aliases,
            extension_mapping_cells: Vec::new(),
            repo_env_json,
        })
    }

    pub fn root_module_name(&self) -> &str {
        &self.root_module_name
    }

    pub fn extension_aggregations(&self) -> &HashMap<String, AggregatedExtension> {
        &self.aggregated_extensions
    }

    pub fn repo_mappings(&self) -> &crate::RepoMappingSnapshot {
        &self.repo_mappings
    }

    pub fn repo_mapping_overrides(&self) -> &crate::RepoMappingOverrides {
        &self.repo_mapping_overrides
    }

    pub async fn resolve_use_repo_rule_local_bits(
        &mut self,
        ctx: &mut DiceComputations<'_>,
        parsed_modules: &[(String, ParsedModuleFile)],
    ) -> slug_error::Result<()> {
        resolve_use_repo_rule_local_bits(
            ctx,
            &mut self.pre_computed_cells,
            parsed_modules,
            &self.root_module_name,
        )
        .await
    }

    pub fn install_precomputed_extension_mapping_rows(&mut self) {
        self.extension_mapping_cells = self.pre_computed_cells.clone();
        self.add_extension_repo_mapping_rows_from_current_cells();
    }

    pub fn finish(mut self) -> slug_error::Result<BzlmodCellGraphValue> {
        add_scoped_repo_aliases_from_mapping_snapshot(
            &mut self.scoped_aliases,
            &self.repo_mappings,
        );

        let mut extension_cells = Vec::new();
        for cell in self.pre_computed_cells {
            extension_cells.push(BzlmodCellGraphExtensionCell {
                canonical_name: cell.canonical_name,
                internal_name: cell.internal_name,
                path: cell.path,
                extension_id: cell.extension_id,
                spec_hash: cell.spec_hash,
                repo_spec_json: cell.repo_spec_json,
                repo_env_json: self.repo_env_json.clone(),
                extension_usages_digest: String::new(),
                extension_replay_inputs_identity_digest: String::new(),
                extension_repo_mappings_digest: String::new(),
                extension_repo_mapping_overrides_digest: String::new(),
                extension_bzl_transitive_digest: String::new(),
                extension_recorded_inputs_json: String::new(),
                materialized: false,
                lazy: false,
            });
        }

        let mut existing_cell_names: HashSet<String> =
            self.cells.iter().map(|cell| cell.name.clone()).collect();
        let mut extension_root_aliases = Vec::new();
        for alias in self.pre_computed_aliases {
            let target_name = selected_bzlmod_cell_name_for_dep_in_graph_cells(
                &self.cells,
                &alias.canonical_name,
                &self.selected_module_versions,
            )
            .unwrap_or(alias.canonical_name.as_str())
            .to_owned();
            let is_generated_override_alias = alias.declaring_module.is_none()
                && alias.apparent_name != target_name
                && crate::pending_repo_cells::parse_canonical_name(&alias.apparent_name).is_some();
            let is_root_declared_alias =
                alias.declaring_module.as_deref() == Some(&self.root_module_name);
            if let Some(owner_module) = alias.declaring_module.as_deref().or_else(|| {
                crate::pending_repo_cells::parse_canonical_name(&alias.canonical_name)
                    .map(|(owner_module, _, _)| owner_module)
            }) {
                self.scoped_aliases.push(BzlmodCellGraphScopedAlias {
                    owner_module: owner_module.to_owned(),
                    apparent_name: alias.apparent_name.clone(),
                    target_name: target_name.clone(),
                });
            }
            if is_generated_override_alias {
                if let Some(dynamic_alias) =
                    dynamic_alias_for_generated_override(&alias, &target_name)
                {
                    self.dynamic_aliases.push(dynamic_alias);
                }
                if !existing_cell_names.contains(&alias.apparent_name) {
                    if let Some(selected_cell) =
                        self.cells.iter().find(|cell| cell.name == target_name)
                    {
                        let selected_source_path = selected_cell
                            .module_setup
                            .as_ref()
                            .map(|setup| PathBuf::from(&setup.source_path))
                            .unwrap_or_else(|| {
                                self.workspace_id
                                    .canonical_project_root
                                    .join(&selected_cell.path)
                            });
                        let selected_setup = selected_cell.module_setup.clone().or_else(|| {
                            Some(BzlmodCellGraphModuleSetup {
                                module_name: target_name.clone(),
                                version: String::new(),
                                registry_url: "override_repo".to_owned(),
                                source_path: selected_source_path.to_string_lossy().into_owned(),
                            })
                        });
                        self.cells.push(BzlmodCellGraphCell {
                            name: alias.apparent_name.clone(),
                            path: format!("bazel-external/{}", alias.apparent_name.as_str()),
                            module_setup: selected_setup,
                            bundled: false,
                        });
                        self.module_symlinks.push(BzlmodCellGraphModuleSymlink {
                            entry_name: alias.apparent_name.clone(),
                            source_path: Arc::new(selected_source_path),
                        });
                        existing_cell_names.insert(alias.apparent_name.clone());
                    } else {
                        tracing::warn!(
                            "override_repo generated repo '{}' targets '{}', but the selected cell was not registered",
                            alias.apparent_name,
                            target_name
                        );
                    }
                }
                extension_cells.retain(|cell| cell.canonical_name != alias.apparent_name);
            }
            if !is_root_declared_alias {
                continue;
            }
            if existing_cell_names.contains(alias.apparent_name.as_str()) {
                continue;
            }
            extension_root_aliases.push(BzlmodCellGraphAlias {
                apparent_name: alias.apparent_name,
                target_name,
            });
        }
        add_scoped_repo_aliases_from_root_overrides(
            &mut self.scoped_aliases,
            &self.repo_mapping_overrides,
            &self.root_module_name,
            &self.cells,
            &self.selected_module_versions,
        );
        self.root_aliases.extend(extension_root_aliases);

        Ok(BzlmodCellGraphValue {
            workspace_id: self.workspace_id,
            root_module_name: self.root_module_name,
            cells: Arc::new(
                self.cells
                    .into_iter()
                    .chain(
                        BZLMOD_ALWAYS_BUNDLED_CELLS
                            .iter()
                            .map(|name| BzlmodCellGraphCell {
                                name: (*name).to_owned(),
                                path: (*name).to_owned(),
                                module_setup: None,
                                bundled: true,
                            }),
                    )
                    .collect(),
            ),
            extension_cells: Arc::new(extension_cells),
            root_aliases: Arc::new(self.root_aliases),
            module_symlinks: Arc::new(self.module_symlinks),
            scoped_aliases: Arc::new(self.scoped_aliases),
            dynamic_aliases: Arc::new(self.dynamic_aliases),
        })
    }

    fn add_extension_repo_mapping_rows_from_current_cells(&mut self) {
        let mut by_extension: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for cell in &self.extension_mapping_cells {
            by_extension
                .entry(cell.extension_id.clone())
                .or_default()
                .push((cell.internal_name.clone(), cell.canonical_name.clone()));
        }

        for (extension_id, generated_repos) in by_extension {
            let overrides = self.repo_mapping_overrides.get(&extension_id);
            if !crate::repo_mapping::add_extension_generated_repo_mappings(
                &mut self.repo_mappings,
                &extension_id,
                &self.root_module_name,
                generated_repos,
                overrides,
            ) {
                tracing::debug!(
                    "Skipping extension repo mapping rows for '{}': owner module mapping is unavailable",
                    extension_id
                );
            }
        }
    }
}

async fn resolve_use_repo_rule_local_bits(
    ctx: &mut DiceComputations<'_>,
    cells: &mut [crate::pending_repo_cells::PendingRepoCell],
    parsed_modules: &[(String, ParsedModuleFile)],
    root_module_name: &str,
) -> slug_error::Result<()> {
    let Ok(executor) = crate::STARLARK_REPO_RULE_EXECUTOR_IMPL.get() else {
        return Ok(());
    };

    let mut declaring_cells = HashMap::new();
    for (_cell_name, parsed) in parsed_modules {
        let module_name = if parsed.module.name.is_empty() {
            root_module_name
        } else {
            &parsed.module.name
        };
        for invocation in &parsed.repo_rule_invocations {
            let rule_source = canonicalize_use_repo_rule_source(
                &invocation.rule_source,
                module_name,
                module_name == root_module_name,
            );
            declaring_cells.insert(rule_source, module_name.to_owned());
        }
    }

    for cell in cells {
        if cell.repo_spec_json.is_empty() {
            continue;
        }

        let mut repo_spec: RepoSpec = serde_json::from_str(&cell.repo_spec_json)
            .with_buck_error_context(|| {
                format!(
                    "Failed to parse precomputed RepoSpec for '{}'",
                    cell.canonical_name
                )
            })?;
        if repo_spec.local || cell.extension_id != repo_spec.repo_rule_id {
            continue;
        }

        let Some((rule_bzl_path, rule_name)) = repo_spec.repo_rule_id.rsplit_once('%') else {
            continue;
        };
        if rule_bzl_path.starts_with('@') {
            continue;
        }
        if !declaring_cells.contains_key(&repo_spec.repo_rule_id) {
            continue;
        }
        let is_local = match executor.rule_is_local(ctx, rule_bzl_path, rule_name).await {
            Ok(is_local) => is_local,
            Err(e) => {
                tracing::debug!(
                    "Failed to precompute repository rule local bit for '{}' on '{}': {}; \
                     deferring to normal repository rule execution",
                    repo_spec.repo_rule_id,
                    cell.canonical_name,
                    e
                );
                false
            }
        };
        if is_local {
            repo_spec.local = true;
            cell.spec_hash = repo_spec.compute_hash();
            cell.repo_spec_json =
                serde_json::to_string(&repo_spec).with_buck_error_context(|| {
                    format!(
                        "Failed to serialize local RepoSpec for '{}'",
                        cell.canonical_name
                    )
                })?;
        }
    }

    Ok(())
}

fn canonicalize_use_repo_rule_source(
    rule_source: &str,
    module_name: &str,
    is_root: bool,
) -> String {
    let Some((bzl_file, rule_name)) = rule_source.rsplit_once('%') else {
        return rule_source.to_owned();
    };

    let resolved = if !is_root {
        if bzl_file.starts_with("//") {
            format!("@{}{}", module_name, bzl_file)
        } else if let Some(rest) = bzl_file.strip_prefix(':') {
            format!("@{}//:{}", module_name, rest)
        } else {
            bzl_file.to_owned()
        }
    } else if let Some(rest) = bzl_file.strip_prefix(':') {
        format!("//:{}", rest)
    } else {
        bzl_file.to_owned()
    };

    format!("{resolved}%{rule_name}")
}

fn local_override_cell_path_and_symlink(
    project_root: &Path,
    module_name: &str,
    module_version: &str,
    override_path: &str,
) -> (String, Option<BzlmodCellGraphModuleSymlink>) {
    let module_dir = local_override_module_dir(project_root, override_path);
    if let Ok(project_relative) = module_dir.strip_prefix(project_root) {
        if !project_relative.as_os_str().is_empty() {
            return (project_relative.to_string_lossy().into_owned(), None);
        }
    }

    let canonical_repo = bazel_canonical_module_repo_name(module_name, module_version);
    let source_path = module_dir
        .canonicalize()
        .unwrap_or_else(|_| module_dir.clone());
    (
        format!("bazel-external/{canonical_repo}"),
        Some(BzlmodCellGraphModuleSymlink {
            entry_name: canonical_repo,
            source_path: Arc::new(source_path),
        }),
    )
}

fn cell_name_strs_from_graph_cells(cells: &[BzlmodCellGraphCell]) -> Vec<&str> {
    cells.iter().map(|cell| cell.name.as_str()).collect()
}

fn selected_module_versions_from_graph(graph: &ResolvedGraph) -> HashMap<String, String> {
    let mut selected = graph.selected_versions.clone();
    for (module_name, module_info) in &graph.modules {
        selected.insert(module_name.clone(), module_info.version.clone());
    }
    selected
}

fn selected_bzlmod_cell_name_for_dep_in_graph_cells<'a>(
    cells: &'a [BzlmodCellGraphCell],
    dep_name: &str,
    selected_module_versions: &HashMap<String, String>,
) -> Option<&'a str> {
    if let Some(cell) = cells.iter().find(|cell| cell.name == dep_name) {
        return Some(cell.name.as_str());
    }

    let selected_version = selected_module_versions.get(dep_name)?;
    let canonical_name = bazel_canonical_module_repo_name(dep_name, selected_version);
    cells
        .iter()
        .find(|cell| cell.name == canonical_name)
        .map(|cell| cell.name.as_str())
}

fn add_scoped_repo_aliases_from_mapping_snapshot(
    aliases: &mut Vec<BzlmodCellGraphScopedAlias>,
    snapshot: &crate::RepoMappingSnapshot,
) {
    for (source_repo, mappings) in snapshot {
        if source_repo.is_empty() {
            continue;
        }
        for (apparent_name, target_name) in mappings {
            aliases.push(BzlmodCellGraphScopedAlias {
                owner_module: source_repo.clone(),
                apparent_name: apparent_name.clone(),
                target_name: target_name.clone(),
            });
        }
    }
}

fn add_scoped_repo_aliases_from_root_overrides(
    aliases: &mut Vec<BzlmodCellGraphScopedAlias>,
    repo_mapping_overrides: &crate::RepoMappingOverrides,
    root_module_name: &str,
    cells: &[BzlmodCellGraphCell],
    selected_module_versions: &HashMap<String, String>,
) {
    for (extension_id, overrides) in repo_mapping_overrides {
        let owner_module =
            crate::extension_execution_dice::extract_owning_module(extension_id, root_module_name);
        for (generated_name, replacement_repo) in overrides {
            let target_name = selected_bzlmod_cell_name_for_dep_in_graph_cells(
                cells,
                replacement_repo,
                selected_module_versions,
            )
            .unwrap_or(replacement_repo.as_str())
            .to_owned();
            aliases.push(BzlmodCellGraphScopedAlias {
                owner_module: owner_module.clone(),
                apparent_name: generated_name.clone(),
                target_name: target_name.clone(),
            });
            if let Some(owner_without_separator) = owner_module.strip_suffix('+') {
                aliases.push(BzlmodCellGraphScopedAlias {
                    owner_module: owner_without_separator.to_owned(),
                    apparent_name: generated_name.clone(),
                    target_name,
                });
            }
        }
    }
}

fn dynamic_alias_for_generated_override(
    alias: &crate::pending_repo_cells::RepoAlias,
    target_name: &str,
) -> Option<BzlmodCellGraphDynamicAlias> {
    if alias.declaring_module.is_none()
        && alias.apparent_name != target_name
        && crate::pending_repo_cells::parse_canonical_name(&alias.apparent_name).is_some()
    {
        Some(BzlmodCellGraphDynamicAlias {
            apparent_name: alias.apparent_name.clone(),
            canonical_name: target_name.to_owned(),
        })
    } else {
        None
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub(crate) struct BzlmodCellGraphDataValue {
    pub(crate) workspace_id: WorkspaceId,
    pub(crate) resolution_digest: Arc<str>,
    #[allocative(skip)]
    pub(crate) fallback_cell_graph: Option<Arc<BzlmodCellGraphValue>>,
}

#[cfg(test)]
impl BzlmodCellGraphDataValue {
    #[cfg(test)]
    pub(crate) fn for_workspace(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
        cell_graph: Arc<BzlmodCellGraphValue>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
            fallback_cell_graph: Some(cell_graph),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_workspace_with_resolved_graph(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
        cell_graph: Arc<BzlmodCellGraphValue>,
        _resolved_graph: Option<Arc<ResolvedGraph>>,
    ) -> Self {
        Self::for_workspace_with_resolved_graph_and_fallback(
            workspace_id,
            resolution_digest,
            Some(cell_graph),
        )
    }

    pub(crate) fn for_workspace_with_resolved_graph_and_fallback(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
        fallback_cell_graph: Option<Arc<BzlmodCellGraphValue>>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
            fallback_cell_graph,
        }
    }
}

#[cfg(test)]
#[derive(derive_more::Display, Debug, Hash, Eq, Clone, PartialEq, Allocative)]
#[display("BzlmodCellGraphDataKey")]
pub(crate) struct BzlmodCellGraphDataKey;

#[cfg(test)]
impl dice::InjectedKey for BzlmodCellGraphDataKey {
    type Value = Arc<BzlmodCellGraphDataValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodResolvedModuleSource {
    pub module_name: String,
    pub version: String,
    pub source: ModuleSource,
    pub source_path: Option<PathBuf>,
}

pub fn resolved_module_sources_from_graph(
    graph: &ResolvedGraph,
) -> Vec<BzlmodResolvedModuleSource> {
    let mut modules: Vec<_> = graph.modules.iter().collect();
    modules.sort_by(|left, right| left.0.cmp(right.0));
    modules
        .into_iter()
        .map(|(module_name, module_info)| BzlmodResolvedModuleSource {
            module_name: module_name.clone(),
            version: module_info.version.clone(),
            source: module_info.source.clone(),
            source_path: module_info.source_path.clone(),
        })
        .collect()
}

pub fn resolved_graph_cell_names(graph: &ResolvedGraph) -> Vec<String> {
    let mut modules: Vec<_> = graph.modules.iter().collect();
    modules.sort_by(|left, right| left.0.cmp(right.0));
    modules
        .into_iter()
        .map(|(module_name, module_info)| {
            bazel_canonical_module_repo_name(module_name, &module_info.version)
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub(crate) struct BzlmodModuleSourcesDataValue {
    pub(crate) workspace_id: WorkspaceId,
    pub(crate) resolution_digest: Arc<str>,
    pub(crate) modules: Arc<Vec<BzlmodResolvedModuleSource>>,
}

impl BzlmodModuleSourcesDataValue {
    pub(crate) fn for_workspace(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
        modules: Arc<Vec<BzlmodResolvedModuleSource>>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
            modules,
        }
    }
}

#[derive(derive_more::Display, Debug, Hash, Eq, Clone, PartialEq, Allocative)]
#[display("BzlmodModuleSourcesDataKey")]
pub(crate) struct BzlmodModuleSourcesDataKey;

impl dice::InjectedKey for BzlmodModuleSourcesDataKey {
    type Value = Arc<BzlmodModuleSourcesDataValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub(crate) struct BzlmodCurrentCellGraphValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

#[derive(derive_more::Display, Debug, Hash, Eq, Clone, PartialEq, Allocative)]
#[display("BzlmodCurrentCellGraphKey")]
pub(crate) struct BzlmodCurrentCellGraphKey;

#[async_trait]
impl Key for BzlmodCurrentCellGraphKey {
    type Value = slug_error::Result<Arc<BzlmodCurrentCellGraphValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let module_sources = ctx.compute(&BzlmodModuleSourcesDataKey).await?;
        if module_sources.resolution_digest.as_ref() != INJECTED_BZLMOD_PROJECTION_DIGEST {
            return Ok(Arc::new(BzlmodCurrentCellGraphValue {
                workspace_id: module_sources.workspace_id.clone(),
                resolution_digest: module_sources.resolution_digest.clone(),
            }));
        }

        #[cfg(not(test))]
        {
            return Err(injected_cell_graph_fallback_disabled_error(
                "BzlmodCurrentCellGraphKey",
            ));
        }

        #[cfg(test)]
        {
            let data = ctx.compute(&BzlmodCellGraphDataKey).await?;
            if data.workspace_id != module_sources.workspace_id {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "BzlmodCurrentCellGraphKey found module-source data for project root '{}', \
                 but fallback cell graph data for project root '{}'",
                    module_sources.workspace_id.canonical_project_root.display(),
                    data.workspace_id.canonical_project_root.display()
                ));
            }
            if data.resolution_digest != module_sources.resolution_digest {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "BzlmodCurrentCellGraphKey found module-source digest '{}', \
                 but fallback cell graph digest '{}'",
                    module_sources.resolution_digest,
                    data.resolution_digest
                ));
            }
            Ok(Arc::new(BzlmodCurrentCellGraphValue {
                workspace_id: data.workspace_id.clone(),
                resolution_digest: data.resolution_digest.clone(),
            }))
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Allocative)]
pub struct BzlmodResolvedGraphOutputsValue {
    #[allocative(skip)]
    pub graph: Arc<ResolvedGraph>,
    pub graph_digest: Arc<str>,
    pub cell_graph_resolution_digest: Arc<str>,
    pub module_versions: BzlmodModuleVersionsDataValue,
    pub resolution_facts: BzlmodResolutionFactsValue,
    pub registered_toolchains: RegisteredToolchainsDataValue,
    pub registered_execution_platforms: RegisteredExecutionPlatformsDataValue,
    pub extension_aggregations: BzlmodExtensionAggregationsDataValue,
    pub repo_mappings: BzlmodRepoMappingsDataValue,
    pub cell_graph: BzlmodCellGraphValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
struct BzlmodModuleSourcesValue {
    modules: Arc<Vec<BzlmodResolvedModuleSource>>,
}

#[derive(derive_more::Display, Debug, Hash, Eq, Clone, PartialEq, Allocative)]
#[display(
    "BzlmodModuleSourcesKey({}, {})",
    workspace_id.stable_hash(),
    resolution_digest
)]
struct BzlmodModuleSourcesKey {
    workspace_id: WorkspaceId,
    resolution_digest: Arc<str>,
}

#[async_trait]
impl Key for BzlmodModuleSourcesKey {
    type Value = slug_error::Result<Arc<BzlmodModuleSourcesValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let data = ctx.compute(&BzlmodModuleSourcesDataKey).await?;
        validate_module_sources_payload(
            "BzlmodModuleSourcesKey",
            &self.workspace_id,
            &self.resolution_digest,
            &data,
        )?;
        Ok(Arc::new(BzlmodModuleSourcesValue {
            modules: data.modules.dupe(),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Allocative)]
struct BzlmodFallbackCellGraphValue {
    fallback_cell_graph: Option<Arc<BzlmodCellGraphValue>>,
}

#[derive(derive_more::Display, Debug, Hash, Eq, Clone, PartialEq, Allocative)]
#[display(
    "BzlmodFallbackCellGraphKey({}, {})",
    workspace_id.stable_hash(),
    resolution_digest
)]
struct BzlmodFallbackCellGraphKey {
    workspace_id: WorkspaceId,
    resolution_digest: Arc<str>,
}

#[async_trait]
impl Key for BzlmodFallbackCellGraphKey {
    type Value = slug_error::Result<Arc<BzlmodFallbackCellGraphValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        #[cfg(not(test))]
        {
            let _ = ctx;
            return Err(injected_cell_graph_fallback_disabled_error(
                "BzlmodFallbackCellGraphKey",
            ));
        }

        #[cfg(test)]
        {
            let data = ctx.compute(&BzlmodCellGraphDataKey).await?;
            validate_cell_graph_payload(
                "BzlmodFallbackCellGraphKey",
                &self.workspace_id,
                &self.resolution_digest,
                &data,
            )?;
            Ok(Arc::new(BzlmodFallbackCellGraphValue {
                fallback_cell_graph: data.fallback_cell_graph.dupe(),
            }))
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

#[derive(derive_more::Display, Debug, Hash, Eq, Clone, PartialEq, Allocative)]
#[display("BzlmodCellDefinitionsKey")]
struct BzlmodCellDefinitionsKey {
    workspace_id: WorkspaceId,
    resolution_digest: Arc<str>,
}

#[async_trait]
impl Key for BzlmodCellDefinitionsKey {
    type Value = slug_error::Result<Arc<Vec<BzlmodCellGraphCell>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if should_use_empty_bzlmod_cell_graph(&self.resolution_digest) {
            return Ok(Arc::new(Vec::new()));
        }
        if should_use_clean_resolution_data(&self.resolution_digest) {
            let module_sources = ctx
                .compute(&BzlmodModuleSourcesKey {
                    workspace_id: self.workspace_id.clone(),
                    resolution_digest: self.resolution_digest.clone(),
                })
                .await??;
            let module_versions = ctx
                .compute(&ModuleVersionsKey::for_workspace_id_with_resolution_digest(
                    self.workspace_id.clone(),
                    self.resolution_digest.clone(),
                ))
                .await??;
            let repo_mappings = ctx
                .compute(
                    &BzlmodRepoMappingsKey::for_workspace_id_with_resolution_digest(
                        self.workspace_id.clone(),
                        self.resolution_digest.clone(),
                    ),
                )
                .await??;
            return Ok(Arc::new(module_cells_from_module_sources(
                &self.workspace_id,
                &module_versions.invalidation.root_module_name,
                module_sources.modules.as_ref(),
                &repo_mappings,
            )));
        }
        let data = ctx
            .compute(&BzlmodFallbackCellGraphKey {
                workspace_id: self.workspace_id.clone(),
                resolution_digest: self.resolution_digest.clone(),
            })
            .await??;
        Ok(data
            .fallback_cell_graph
            .as_ref()
            .map_or_else(|| Arc::new(Vec::new()), |graph| graph.cells.dupe()))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

#[derive(derive_more::Display, Debug, Hash, Eq, Clone, PartialEq, Allocative)]
#[display("BzlmodExtensionCellDefinitionsKey")]
struct BzlmodExtensionCellDefinitionsKey {
    workspace_id: WorkspaceId,
    resolution_digest: Arc<str>,
}

#[async_trait]
impl Key for BzlmodExtensionCellDefinitionsKey {
    type Value = slug_error::Result<Arc<Vec<BzlmodCellGraphExtensionCell>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if should_use_empty_bzlmod_cell_graph(&self.resolution_digest) {
            return Ok(Arc::new(Vec::new()));
        }
        let extension_aggregations = ctx.compute(&BzlmodExtensionAggregationsDataKey).await?;
        validate_extension_aggregations_payload(
            "BzlmodExtensionCellDefinitionsKey",
            &self.workspace_id,
            &self.resolution_digest,
            extension_aggregations.as_ref(),
            "extension cells",
        )?;
        if should_use_clean_resolution_data(&self.resolution_digest) {
            return Ok(extension_aggregations.declared_extension_cells.dupe());
        }
        let declared_cells =
            fallback_extension_cells(ctx, &self.workspace_id, self.resolution_digest.clone())
                .await?;
        if !extension_aggregations.extension_aggregations.is_empty() {
            let repo_env = ctx
                .compute(&BzlmodRepoEnvKey::for_workspace_id_with_resolution_digest(
                    self.workspace_id.clone(),
                    self.resolution_digest.clone(),
                ))
                .await??;
            let repo_mappings = ctx
                .compute(
                    &BzlmodRepoMappingsKey::for_workspace_id_with_resolution_digest(
                        self.workspace_id.clone(),
                        self.resolution_digest.clone(),
                    ),
                )
                .await??;
            return match extension_cells_from_spokes(
                ctx,
                &self.workspace_id,
                &self.resolution_digest,
                extension_aggregations.as_ref(),
                repo_env.as_ref(),
                repo_mappings.as_ref(),
            )
            .await
            {
                Ok(cells) => Ok(merge_declared_and_spoke_extension_cells(
                    declared_cells,
                    cells,
                )),
                Err(e) if e.to_string().contains("module extension executor") => {
                    #[cfg(test)]
                    {
                        let _ = e;
                        Ok(declared_cells)
                    }
                    #[cfg(not(test))]
                    {
                        Err(e)
                    }
                }
                Err(e) => Err(e),
            };
        }
        Ok(declared_cells)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

async fn fallback_extension_cells(
    ctx: &mut DiceComputations<'_>,
    workspace_id: &WorkspaceId,
    resolution_digest: Arc<str>,
) -> slug_error::Result<Arc<Vec<BzlmodCellGraphExtensionCell>>> {
    let data = ctx
        .compute(&BzlmodFallbackCellGraphKey {
            workspace_id: workspace_id.clone(),
            resolution_digest,
        })
        .await??;
    Ok(data.fallback_cell_graph.as_ref().map_or_else(
        || Arc::new(Vec::new()),
        |graph| graph.extension_cells.dupe(),
    ))
}

fn merge_declared_and_spoke_extension_cells(
    declared: Arc<Vec<BzlmodCellGraphExtensionCell>>,
    spokes: Arc<Vec<BzlmodCellGraphExtensionCell>>,
) -> Arc<Vec<BzlmodCellGraphExtensionCell>> {
    let mut seen = BTreeSet::new();
    let mut merged = Vec::with_capacity(spokes.len() + declared.len());
    for cell in spokes.iter() {
        seen.insert(cell.canonical_name.clone());
        merged.push(cell.clone());
    }
    for cell in declared.iter() {
        if seen.insert(cell.canonical_name.clone()) {
            merged.push(cell.clone());
        }
    }
    Arc::new(merged)
}

async fn extension_cells_from_spokes(
    ctx: &mut DiceComputations<'_>,
    workspace_id: &WorkspaceId,
    resolution_digest: &Arc<str>,
    extension_aggregations: &BzlmodExtensionAggregationsDataValue,
    repo_env: &BTreeMap<String, String>,
    repo_mappings: &BzlmodRepoMappingsDataValue,
) -> slug_error::Result<Arc<Vec<BzlmodCellGraphExtensionCell>>> {
    let mut cells = Vec::new();
    let generated_override_aliases = generated_override_aliases(repo_mappings);
    let mut extension_ids: Vec<_> = extension_aggregations
        .extension_aggregations
        .keys()
        .collect();
    extension_ids.sort();
    let repo_env_json = serde_json::to_string(repo_env).map_err(|e| {
        slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "Failed to serialize bzlmod repo_env for extension cells: {}",
            e
        )
    })?;
    for extension_id in extension_ids {
        let spokes = ctx
            .compute(
                &ExtensionSpokesByExtensionIdKey::for_workspace_id_with_resolution_digest(
                    workspace_id.clone(),
                    resolution_digest.clone(),
                    extension_id,
                ),
            )
            .await??;
        let Some(spokes) = spokes else {
            continue;
        };
        for spoke in spokes.iter() {
            if generated_override_aliases.contains(spoke.canonical_name.as_ref())
                || generated_override_aliases.contains(spoke.internal_name.as_ref())
            {
                continue;
            }
            cells.push(BzlmodCellGraphExtensionCell {
                canonical_name: spoke.canonical_name.to_string(),
                internal_name: spoke.internal_name.to_string(),
                path: format!("bazel-external/{}", spoke.canonical_name),
                extension_id: extension_id.clone(),
                spec_hash: spoke.spec_hash.to_string(),
                repo_spec_json: spoke.repo_spec_json.to_string(),
                repo_env_json: repo_env_json.clone(),
                extension_usages_digest: spokes.usages_digest.to_string(),
                extension_replay_inputs_identity_digest: spokes
                    .replay_inputs_identity_digest
                    .to_string(),
                extension_repo_mappings_digest: spokes.repo_mappings_digest.to_string(),
                extension_repo_mapping_overrides_digest: spokes
                    .repo_mapping_overrides_digest
                    .to_string(),
                extension_bzl_transitive_digest: spokes.bzl_transitive_digest.to_string(),
                extension_recorded_inputs_json: serde_json::to_string(
                    spokes.recorded_inputs.as_ref(),
                )
                .unwrap_or_else(|_| "[]".to_owned()),
                materialized: false,
                lazy: false,
            });
        }
    }
    Ok(Arc::new(cells))
}

fn generated_override_aliases(repo_mappings: &BzlmodRepoMappingsDataValue) -> BTreeSet<String> {
    repo_mappings
        .repo_mappings
        .get("")
        .into_iter()
        .flat_map(|mapping| mapping.iter())
        .filter_map(|(apparent_name, target_name)| {
            (apparent_name != target_name
                && crate::pending_repo_cells::parse_canonical_name(apparent_name).is_some())
            .then_some(apparent_name.clone())
        })
        .collect()
}

#[derive(derive_more::Display, Debug, Hash, Eq, Clone, PartialEq, Allocative)]
#[display("BzlmodResidualModuleSymlinksKey")]
struct BzlmodResidualModuleSymlinksKey {
    workspace_id: WorkspaceId,
    resolution_digest: Arc<str>,
}

#[async_trait]
impl Key for BzlmodResidualModuleSymlinksKey {
    type Value = slug_error::Result<Arc<Vec<BzlmodCellGraphModuleSymlink>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if should_use_empty_bzlmod_cell_graph(&self.resolution_digest) {
            return Ok(Arc::new(Vec::new()));
        }
        if should_use_clean_resolution_data(&self.resolution_digest) {
            let module_sources = ctx
                .compute(&BzlmodModuleSourcesKey {
                    workspace_id: self.workspace_id.clone(),
                    resolution_digest: self.resolution_digest.clone(),
                })
                .await??;
            return Ok(Arc::new(residual_module_symlinks_from_module_sources(
                &self.workspace_id,
                module_sources.modules.as_ref(),
            )));
        }
        let data = ctx
            .compute(&BzlmodFallbackCellGraphKey {
                workspace_id: self.workspace_id.clone(),
                resolution_digest: self.resolution_digest.clone(),
            })
            .await??;
        Ok(Arc::new(
            data.fallback_cell_graph
                .as_ref()
                .map_or_else(Vec::new, |graph| {
                    residual_module_symlinks_from_payload(graph.as_ref())
                }),
        ))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

#[cfg(test)]
fn validate_cell_graph_payload(
    key_name: &str,
    workspace_id: &WorkspaceId,
    resolution_digest: &str,
    data: &BzlmodCellGraphDataValue,
) -> slug_error::Result<()> {
    if data.workspace_id != *workspace_id {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "{} was computed with project root '{}', \
             but current bzlmod cell graph root is '{}'",
            key_name,
            workspace_id.canonical_project_root.display(),
            data.workspace_id.canonical_project_root.display()
        ));
    }
    if data.resolution_digest.as_ref() != resolution_digest {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "{} was computed with resolution digest '{}', \
             but current bzlmod cell graph digest is '{}'",
            key_name,
            resolution_digest,
            data.resolution_digest
        ));
    }
    Ok(())
}

#[cfg(not(test))]
fn injected_cell_graph_fallback_disabled_error(key_name: &str) -> slug_error::Error {
    slug_error::slug_error!(
        slug_error::ErrorTag::Tier0,
        "{} cannot use injected bzlmod cell graph fallback in non-test builds",
        key_name
    )
}

fn should_use_clean_resolution_data(resolution_digest: &str) -> bool {
    resolution_digest != INJECTED_BZLMOD_PROJECTION_DIGEST
        && resolution_digest != EMPTY_BZLMOD_CELL_GRAPH_DIGEST
}

fn should_use_empty_bzlmod_cell_graph(resolution_digest: &str) -> bool {
    resolution_digest == EMPTY_BZLMOD_CELL_GRAPH_DIGEST
}

fn validate_module_sources_payload(
    key_name: &str,
    workspace_id: &WorkspaceId,
    resolution_digest: &str,
    data: &BzlmodModuleSourcesDataValue,
) -> slug_error::Result<()> {
    if data.workspace_id != *workspace_id {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "{} was computed with project root '{}', \
             but current bzlmod module-source root is '{}'",
            key_name,
            workspace_id.canonical_project_root.display(),
            data.workspace_id.canonical_project_root.display()
        ));
    }
    if data.resolution_digest.as_ref() != resolution_digest {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "{} was computed with resolution digest '{}', \
             but current bzlmod module-source digest is '{}'",
            key_name,
            resolution_digest,
            data.resolution_digest
        ));
    }
    Ok(())
}

pub(crate) fn validate_extension_aggregations_payload(
    key_name: &str,
    workspace_id: &WorkspaceId,
    resolution_digest: &str,
    data: &BzlmodExtensionAggregationsDataValue,
    subject: &str,
) -> slug_error::Result<()> {
    if data.workspace_id != *workspace_id {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "{} for '{}' was computed with project root '{}', \
             but current bzlmod extension aggregation data root is '{}'",
            key_name,
            subject,
            workspace_id.canonical_project_root.display(),
            data.workspace_id.canonical_project_root.display()
        ));
    }
    if data.resolution_digest.as_ref() != resolution_digest {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "{} for '{}' was computed with resolution digest '{}', \
             but current bzlmod extension aggregation data digest is '{}'",
            key_name,
            subject,
            resolution_digest,
            data.resolution_digest
        ));
    }
    Ok(())
}

#[async_trait]
impl Key for BzlmodCellGraphKey {
    type Value = slug_error::Result<Arc<BzlmodCellGraphValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let cells = ctx
            .compute(&BzlmodCellDefinitionsKey {
                workspace_id: self.workspace_id.clone(),
                resolution_digest: self.resolution_digest.clone(),
            })
            .await??;
        let extension_cells = ctx
            .compute(&BzlmodExtensionCellDefinitionsKey {
                workspace_id: self.workspace_id.clone(),
                resolution_digest: self.resolution_digest.clone(),
            })
            .await??;
        let residual_module_symlinks = ctx
            .compute(&BzlmodResidualModuleSymlinksKey {
                workspace_id: self.workspace_id.clone(),
                resolution_digest: self.resolution_digest.clone(),
            })
            .await??;
        let module_versions = ctx
            .compute(&ModuleVersionsKey::for_workspace_id_with_resolution_digest(
                self.workspace_id.clone(),
                self.resolution_digest.clone(),
            ))
            .await??;
        let repo_mappings = ctx
            .compute(
                &BzlmodRepoMappingsKey::for_workspace_id_with_resolution_digest(
                    self.workspace_id.clone(),
                    self.resolution_digest.clone(),
                ),
            )
            .await??;
        let extension_aggregations = ctx.compute(&BzlmodExtensionAggregationsDataKey).await?;
        validate_extension_aggregations_payload(
            "BzlmodCellGraphKey",
            &self.workspace_id,
            &self.resolution_digest,
            extension_aggregations.as_ref(),
            "cell graph",
        )?;
        let root_module_name = module_versions.invalidation.root_module_name.clone();
        let root_aliases = merge_declared_root_aliases(
            root_aliases_from_repo_mappings(&repo_mappings),
            repo_mappings.declared_root_aliases.as_ref(),
        );
        let scoped_aliases = merge_declared_scoped_aliases(
            scoped_aliases_from_repo_mappings(&repo_mappings, &root_module_name),
            repo_mappings.declared_scoped_aliases.as_ref(),
        );
        let dynamic_aliases = merge_declared_dynamic_aliases(
            dynamic_aliases_from_repo_mappings(&repo_mappings),
            repo_mappings.declared_dynamic_aliases.as_ref(),
        );
        Ok(Arc::new(BzlmodCellGraphValue {
            workspace_id: self.workspace_id.clone(),
            root_module_name: root_module_name.clone(),
            cells: cells.dupe(),
            extension_cells,
            root_aliases: Arc::new(root_aliases),
            module_symlinks: Arc::new(module_symlinks_from_cells_and_residuals(
                cells.as_ref(),
                residual_module_symlinks.as_ref(),
            )),
            scoped_aliases: Arc::new(scoped_aliases),
            dynamic_aliases: Arc::new(dynamic_aliases),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

fn residual_module_symlinks_from_payload(
    cell_graph: &BzlmodCellGraphValue,
) -> Vec<BzlmodCellGraphModuleSymlink> {
    let derived: BTreeSet<_> = cell_graph
        .cells
        .iter()
        .filter_map(|cell| {
            let setup = cell.module_setup.as_ref()?;
            if setup.source_path.is_empty() {
                return None;
            }
            Some(cell.name.clone())
        })
        .collect();
    cell_graph
        .module_symlinks
        .iter()
        .filter(|symlink| !derived.contains(&symlink.entry_name))
        .cloned()
        .collect()
}

fn residual_module_symlinks_from_module_sources(
    workspace_id: &WorkspaceId,
    module_sources: &[BzlmodResolvedModuleSource],
) -> Vec<BzlmodCellGraphModuleSymlink> {
    let mut symlinks = Vec::new();
    for module_info in module_sources {
        let ModuleSource::LocalPath { path } = &module_info.source else {
            continue;
        };
        let module_dir =
            local_override_module_dir(workspace_id.canonical_project_root.as_ref(), path);
        if module_dir
            .strip_prefix(workspace_id.canonical_project_root.as_ref())
            .ok()
            .is_some_and(|relative| !relative.as_os_str().is_empty())
        {
            continue;
        }
        let canonical_repo =
            bazel_canonical_module_repo_name(&module_info.module_name, &module_info.version);
        let source_path = module_dir
            .canonicalize()
            .unwrap_or_else(|_| module_dir.clone());
        symlinks.push(BzlmodCellGraphModuleSymlink {
            entry_name: canonical_repo,
            source_path: Arc::new(source_path),
        });
    }
    symlinks
}

fn module_symlinks_from_cells_and_residuals(
    cells: &[BzlmodCellGraphCell],
    residual_module_symlinks: &[BzlmodCellGraphModuleSymlink],
) -> Vec<BzlmodCellGraphModuleSymlink> {
    let mut seen = BTreeSet::new();
    let mut symlinks = Vec::new();
    for cell in cells {
        let Some(setup) = cell.module_setup.as_ref() else {
            continue;
        };
        if setup.source_path.is_empty() {
            continue;
        }
        if seen.insert(cell.name.clone()) {
            symlinks.push(BzlmodCellGraphModuleSymlink {
                entry_name: cell.name.clone(),
                source_path: Arc::new(PathBuf::from(&setup.source_path)),
            });
        }
    }
    for symlink in residual_module_symlinks {
        if seen.insert(symlink.entry_name.clone()) {
            symlinks.push(symlink.clone());
        }
    }
    symlinks
}

const BZLMOD_ALWAYS_BUNDLED_CELLS: &[&str] = &[
    "bazel_tools",
    "local_config_platform",
    "slug_builtins",
    "local_config_python",
];

pub fn bazel_canonical_module_repo_name(module_name: &str, version: &str) -> String {
    if module_name.contains('+') {
        module_name.to_owned()
    } else if version.is_empty() {
        format!("{module_name}+")
    } else {
        format!("{module_name}+")
    }
}

fn module_cells_from_module_sources(
    workspace_id: &WorkspaceId,
    root_module_name: &str,
    module_sources: &[BzlmodResolvedModuleSource],
    repo_mappings: &BzlmodRepoMappingsDataValue,
) -> Vec<BzlmodCellGraphCell> {
    let mut cells = Vec::new();
    for module_info in module_sources {
        let module_name = module_info.module_name.as_str();
        if module_name.is_empty() || module_name == root_module_name {
            continue;
        }

        let canonical_repo = bazel_canonical_module_repo_name(module_name, &module_info.version);
        match &module_info.source {
            ModuleSource::Registry { url } => {
                let source_path = module_info
                    .source_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                cells.push(BzlmodCellGraphCell {
                    name: canonical_repo.clone(),
                    path: format!("bazel-external/{canonical_repo}"),
                    module_setup: Some(BzlmodCellGraphModuleSetup {
                        module_name: module_name.to_owned(),
                        version: module_info.version.clone(),
                        registry_url: url.clone(),
                        source_path,
                    }),
                    bundled: false,
                });
            }
            ModuleSource::LocalPath { path } => {
                cells.push(BzlmodCellGraphCell {
                    name: canonical_repo.clone(),
                    path: local_override_cell_path(
                        workspace_id.canonical_project_root.as_ref(),
                        &canonical_repo,
                        path,
                    ),
                    module_setup: None,
                    bundled: false,
                });
            }
            ModuleSource::Git { remote, .. } => {
                let source_path = module_info
                    .source_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                cells.push(BzlmodCellGraphCell {
                    name: canonical_repo.clone(),
                    path: format!("bazel-external/{canonical_repo}"),
                    module_setup: Some(BzlmodCellGraphModuleSetup {
                        module_name: module_name.to_owned(),
                        version: module_info.version.clone(),
                        registry_url: format!("git+{remote}"),
                        source_path,
                    }),
                    bundled: false,
                });
            }
            ModuleSource::Archive { urls, .. } => {
                let source_path = module_info
                    .source_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let url = urls
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "archive".to_owned());
                cells.push(BzlmodCellGraphCell {
                    name: canonical_repo.clone(),
                    path: format!("bazel-external/{canonical_repo}"),
                    module_setup: Some(BzlmodCellGraphModuleSetup {
                        module_name: module_name.to_owned(),
                        version: module_info.version.clone(),
                        registry_url: url,
                        source_path,
                    }),
                    bundled: false,
                });
            }
        }
    }

    add_generated_override_identity_cells(&mut cells, workspace_id, repo_mappings);

    cells.extend(
        BZLMOD_ALWAYS_BUNDLED_CELLS
            .iter()
            .map(|name| BzlmodCellGraphCell {
                name: (*name).to_owned(),
                path: (*name).to_owned(),
                module_setup: None,
                bundled: true,
            }),
    );
    cells
}

fn add_generated_override_identity_cells(
    cells: &mut Vec<BzlmodCellGraphCell>,
    workspace_id: &WorkspaceId,
    repo_mappings: &BzlmodRepoMappingsDataValue,
) {
    let Some(root_mapping) = repo_mappings.repo_mappings.get("") else {
        return;
    };
    let mut existing: BTreeSet<_> = cells.iter().map(|cell| cell.name.clone()).collect();
    for (apparent_name, target_name) in root_mapping {
        if apparent_name == target_name
            || crate::pending_repo_cells::parse_canonical_name(apparent_name).is_none()
            || existing.contains(apparent_name)
        {
            continue;
        }
        let Some(target_cell) = cells.iter().find(|cell| cell.name == *target_name).cloned() else {
            continue;
        };
        let selected_source_path = target_cell
            .module_setup
            .as_ref()
            .map(|setup| PathBuf::from(&setup.source_path))
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| workspace_id.canonical_project_root.join(&target_cell.path));
        let module_setup = target_cell.module_setup.clone().or_else(|| {
            Some(BzlmodCellGraphModuleSetup {
                module_name: target_name.clone(),
                version: String::new(),
                registry_url: "override_repo".to_owned(),
                source_path: selected_source_path.to_string_lossy().into_owned(),
            })
        });
        cells.push(BzlmodCellGraphCell {
            name: apparent_name.clone(),
            path: format!("bazel-external/{apparent_name}"),
            module_setup,
            bundled: false,
        });
        existing.insert(apparent_name.clone());
    }
}

fn local_override_cell_path(
    project_root: &Path,
    canonical_repo: &str,
    override_path: &str,
) -> String {
    let module_dir = local_override_module_dir(project_root, override_path);
    if let Ok(project_relative) = module_dir.strip_prefix(project_root) {
        if !project_relative.as_os_str().is_empty() {
            return project_relative.to_string_lossy().into_owned();
        }
    }
    format!("bazel-external/{canonical_repo}")
}

fn local_override_module_dir(project_root: &Path, override_path: &str) -> PathBuf {
    let path = Path::new(override_path);
    if path.is_absolute() {
        normalize_path_lexically(path.to_path_buf())
    } else {
        normalize_path_lexically(project_root.join(path))
    }
}

fn normalize_path_lexically(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::Normal(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub enum RepoMappingScope {
    Module(ModuleKey),
    ExtensionImplementation(ModuleExtensionId),
    GeneratedRepo { canonical_repo: Arc<str> },
    InnateRepoRule { invocation_id: Arc<str> },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct RepoMappingKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub scope: RepoMappingScope,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "ModuleVersionsKey({}, {})",
    workspace_id.stable_hash(),
    resolution_digest
)]
pub struct ModuleVersionsKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

impl ModuleVersionsKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
        }
    }

    pub fn for_workspace_id_with_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct ModuleVersionsValue {
    pub workspace_id: WorkspaceId,
    pub module_versions: Arc<HashMap<String, String>>,
    /// Transitional invalidation identity for interpreter state that still
    /// depends on more than the module-name version map.
    pub invalidation: Arc<BzlmodModuleVersionsInvalidation>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "BzlmodLockfileInputsKey({}, {})",
    workspace_id.stable_hash(),
    resolution_digest
)]
pub struct BzlmodLockfileInputsKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

impl BzlmodLockfileInputsKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
        }
    }

    pub fn for_workspace_id_with_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodLockfileInputsDataValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub lockfile_mode: crate::LockfileMode,
    pub hidden_lockfile_path: Option<PathBuf>,
    pub root_module_present: bool,
    #[cfg(test)]
    pub precomputed_lockfile_inputs: Option<Arc<BzlmodLockfileInputsValue>>,
}

impl BzlmodLockfileInputsDataValue {
    #[cfg(test)]
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        lockfile_inputs: Arc<BzlmodLockfileInputsValue>,
    ) -> Self {
        let root_module_present =
            lockfile_inputs.visible_lockfile.is_some() || lockfile_inputs.hidden_lockfile.is_some();
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            lockfile_mode: lockfile_inputs.lockfile_mode,
            hidden_lockfile_path: lockfile_inputs.hidden_lockfile_path.clone(),
            root_module_present,
            precomputed_lockfile_inputs: Some(lockfile_inputs),
        }
    }

    pub fn for_workspace_policy(
        workspace_id: WorkspaceId,
        lockfile_mode: crate::LockfileMode,
        hidden_lockfile_path: Option<PathBuf>,
        root_module_present: bool,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            lockfile_mode,
            hidden_lockfile_path,
            root_module_present,
            #[cfg(test)]
            precomputed_lockfile_inputs: None,
        }
    }

    pub fn with_resolution_digest(mut self, resolution_digest: Arc<str>) -> Self {
        self.resolution_digest = resolution_digest;
        self
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "BzlmodRepoEnvKey({}, {})",
    workspace_id.stable_hash(),
    resolution_digest
)]
pub struct BzlmodRepoEnvKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

impl BzlmodRepoEnvKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
        }
    }

    pub fn for_workspace_id_with_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodRepoEnvDataValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub repo_env: Arc<BTreeMap<String, String>>,
}

impl BzlmodRepoEnvDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        repo_env: Arc<BTreeMap<String, String>>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            repo_env,
        }
    }

    pub fn with_resolution_digest(mut self, resolution_digest: Arc<str>) -> Self {
        self.resolution_digest = resolution_digest;
        self
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "BzlmodRepoMappingsKey({}, {})",
    workspace_id.stable_hash(),
    resolution_digest
)]
pub struct BzlmodRepoMappingsKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

impl BzlmodRepoMappingsKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
        }
    }

    pub fn for_workspace_id_with_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodModuleVersionsInvalidation {
    pub root_module_name: String,
    pub lockfile_inputs: Arc<BzlmodLockfileInputsValue>,
    pub repo_env: BTreeMap<String, String>,
    pub registry_file_hashes: indexmap::IndexMap<String, String>,
    pub selected_yanked_versions: indexmap::IndexMap<String, String>,
    pub repo_mappings: crate::RepoMappingSnapshot,
    pub repo_mapping_overrides: crate::RepoMappingOverrides,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodResolutionFactsValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub registry_file_hashes: indexmap::IndexMap<String, String>,
    pub selected_yanked_versions: indexmap::IndexMap<String, String>,
}

impl BzlmodResolutionFactsValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        registry_file_hashes: indexmap::IndexMap<String, String>,
        selected_yanked_versions: indexmap::IndexMap<String, String>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            registry_file_hashes,
            selected_yanked_versions,
        }
    }

    pub fn with_resolution_digest(mut self, resolution_digest: Arc<str>) -> Self {
        self.resolution_digest = resolution_digest;
        self
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "BzlmodResolutionFactsKey({}, {})",
    workspace_id.stable_hash(),
    resolution_digest
)]
pub struct BzlmodResolutionFactsKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

impl BzlmodResolutionFactsKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
        }
    }

    pub fn for_workspace_id_with_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodModuleVersionsDataValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub root_module_name: String,
    pub module_versions: Arc<HashMap<String, String>>,
}

impl BzlmodModuleVersionsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        module_versions: Arc<HashMap<String, String>>,
    ) -> Self {
        Self::for_workspace_with_root_module_name(workspace_id, String::new(), module_versions)
    }

    pub fn for_workspace_with_root_module_name(
        workspace_id: WorkspaceId,
        root_module_name: String,
        module_versions: Arc<HashMap<String, String>>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            root_module_name,
            module_versions,
        }
    }

    pub fn with_resolution_digest(mut self, resolution_digest: Arc<str>) -> Self {
        self.resolution_digest = resolution_digest;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodRepoMappingsDataValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub repo_mappings: Arc<crate::RepoMappingSnapshot>,
    pub repo_mapping_overrides: Arc<crate::RepoMappingOverrides>,
    pub declared_root_aliases: Arc<Vec<BzlmodCellGraphAlias>>,
    pub declared_scoped_aliases: Arc<Vec<BzlmodCellGraphScopedAlias>>,
    pub declared_dynamic_aliases: Arc<Vec<BzlmodCellGraphDynamicAlias>>,
}

impl BzlmodRepoMappingsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        repo_mappings: Arc<crate::RepoMappingSnapshot>,
        repo_mapping_overrides: Arc<crate::RepoMappingOverrides>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            repo_mappings,
            repo_mapping_overrides,
            declared_root_aliases: Arc::new(Vec::new()),
            declared_scoped_aliases: Arc::new(Vec::new()),
            declared_dynamic_aliases: Arc::new(Vec::new()),
        }
    }

    pub fn with_resolution_digest(mut self, resolution_digest: Arc<str>) -> Self {
        self.resolution_digest = resolution_digest;
        self
    }

    pub fn with_declared_aliases(
        mut self,
        root_aliases: Arc<Vec<BzlmodCellGraphAlias>>,
        scoped_aliases: Arc<Vec<BzlmodCellGraphScopedAlias>>,
        dynamic_aliases: Arc<Vec<BzlmodCellGraphDynamicAlias>>,
    ) -> Self {
        self.declared_root_aliases = root_aliases;
        self.declared_scoped_aliases = scoped_aliases;
        self.declared_dynamic_aliases = dynamic_aliases;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodExtensionAggregationsDataValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub root_module_name: String,
    pub extension_aggregations: Arc<HashMap<String, AggregatedExtension>>,
    pub declared_extension_cells: Arc<Vec<BzlmodCellGraphExtensionCell>>,
}

impl BzlmodExtensionAggregationsDataValue {
    pub fn for_workspace_with_root_module_name(
        workspace_id: WorkspaceId,
        root_module_name: String,
        extension_aggregations: Arc<HashMap<String, AggregatedExtension>>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            root_module_name,
            extension_aggregations,
            declared_extension_cells: Arc::new(Vec::new()),
        }
    }

    pub fn with_resolution_digest(mut self, resolution_digest: Arc<str>) -> Self {
        self.resolution_digest = resolution_digest;
        self
    }

    pub fn with_declared_extension_cells(
        mut self,
        declared_extension_cells: Arc<Vec<BzlmodCellGraphExtensionCell>>,
    ) -> Self {
        self.declared_extension_cells = declared_extension_cells;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodExtensionAggregationValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub extension_id: Arc<str>,
    pub aggregated: Arc<AggregatedExtension>,
    pub root_module_name: Arc<str>,
}

#[derive(
    derive_more::Display,
    Debug,
    Hash,
    Eq,
    Clone,
    Dupe,
    PartialEq,
    Allocative
)]
#[display("BzlmodLockfileInputsDataKey")]
pub struct BzlmodLockfileInputsDataKey;

impl dice::InjectedKey for BzlmodLockfileInputsDataKey {
    type Value = Arc<BzlmodLockfileInputsDataValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(
    derive_more::Display,
    Debug,
    Hash,
    Eq,
    Clone,
    Dupe,
    PartialEq,
    Allocative
)]
#[display("BzlmodRepoEnvDataKey")]
pub struct BzlmodRepoEnvDataKey;

impl dice::InjectedKey for BzlmodRepoEnvDataKey {
    type Value = Arc<BzlmodRepoEnvDataValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(
    derive_more::Display,
    Debug,
    Hash,
    Eq,
    Clone,
    Dupe,
    PartialEq,
    Allocative
)]
#[display("BzlmodExtensionAggregationsDataKey")]
pub struct BzlmodExtensionAggregationsDataKey;

impl dice::InjectedKey for BzlmodExtensionAggregationsDataKey {
    type Value = Arc<BzlmodExtensionAggregationsDataValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(
    derive_more::Display,
    Debug,
    Hash,
    Eq,
    Clone,
    Dupe,
    PartialEq,
    Allocative
)]
#[display("BzlmodRepoMappingsDataKey")]
pub struct BzlmodRepoMappingsDataKey;

impl dice::InjectedKey for BzlmodRepoMappingsDataKey {
    type Value = Arc<BzlmodRepoMappingsDataValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(
    derive_more::Display,
    Debug,
    Hash,
    Eq,
    Clone,
    Dupe,
    PartialEq,
    Allocative
)]
#[display("BzlmodResolutionFactsDataKey")]
pub struct BzlmodResolutionFactsDataKey;

impl dice::InjectedKey for BzlmodResolutionFactsDataKey {
    type Value = Arc<BzlmodResolutionFactsValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(
    derive_more::Display,
    Debug,
    Hash,
    Eq,
    Clone,
    Dupe,
    PartialEq,
    Allocative
)]
#[display("BzlmodModuleVersionsDataKey")]
pub struct BzlmodModuleVersionsDataKey;

impl dice::InjectedKey for BzlmodModuleVersionsDataKey {
    type Value = Arc<BzlmodModuleVersionsDataValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        // Transitional bridge: this narrow value replaces the former direct
        // monolithic projection dependency, but it still carries a conservative
        // invalidation identity until the interpreter dependencies are fully
        // explicit.
        x == y
    }
}

#[async_trait]
impl Key for BzlmodLockfileInputsKey {
    type Value = slug_error::Result<Arc<BzlmodLockfileInputsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let data = ctx.compute(&BzlmodLockfileInputsDataKey).await?;
        if data.workspace_id != self.workspace_id {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "BzlmodLockfileInputsKey was computed with project root '{}', \
                 but current bzlmod lockfile input data root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                data.workspace_id.canonical_project_root.display()
            ));
        }
        if data.resolution_digest != self.resolution_digest {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "BzlmodLockfileInputsKey was computed with resolution digest '{}', \
                 but current bzlmod lockfile input data digest is '{}'",
                self.resolution_digest,
                data.resolution_digest
            ));
        }
        #[cfg(test)]
        if let Some(lockfile_inputs) = data.precomputed_lockfile_inputs.as_ref() {
            return Ok(lockfile_inputs.clone());
        }
        ctx.compute(&BzlmodCleanLockfileInputsKey {
            workspace_id: self.workspace_id.clone(),
            lockfile_mode: data.lockfile_mode,
            hidden_lockfile_path: data.hidden_lockfile_path.clone(),
            root_module_present: data.root_module_present,
        })
        .await?
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        match x {
            Ok(value) => !value.has_untracked_inputs(),
            Err(_) => false,
        }
    }
}

#[async_trait]
impl Key for BzlmodRepoEnvKey {
    type Value = slug_error::Result<Arc<BTreeMap<String, String>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let data = ctx.compute(&BzlmodRepoEnvDataKey).await?;
        if data.workspace_id != self.workspace_id {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "BzlmodRepoEnvKey was computed with project root '{}', \
                 but current bzlmod repo env data root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                data.workspace_id.canonical_project_root.display()
            ));
        }
        if data.resolution_digest != self.resolution_digest {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "BzlmodRepoEnvKey was computed with resolution digest '{}', \
                 but current bzlmod repo env data digest is '{}'",
                self.resolution_digest,
                data.resolution_digest
            ));
        }
        Ok(data.repo_env.clone())
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

#[async_trait]
impl Key for BzlmodRepoMappingsKey {
    type Value = slug_error::Result<Arc<BzlmodRepoMappingsDataValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let data = ctx.compute(&BzlmodRepoMappingsDataKey).await?;
        if data.workspace_id != self.workspace_id {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "BzlmodRepoMappingsKey was computed with project root '{}', \
                 but current bzlmod repo mapping data root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                data.workspace_id.canonical_project_root.display()
            ));
        }
        if data.resolution_digest != self.resolution_digest {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "BzlmodRepoMappingsKey was computed with resolution digest '{}', \
                 but current bzlmod repo mapping data digest is '{}'",
                self.resolution_digest,
                data.resolution_digest
            ));
        }
        Ok(data)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

#[async_trait]
impl Key for BzlmodResolutionFactsKey {
    type Value = slug_error::Result<Arc<BzlmodResolutionFactsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let data = ctx.compute(&BzlmodResolutionFactsDataKey).await?;
        if data.workspace_id != self.workspace_id {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "BzlmodResolutionFactsKey was computed with project root '{}', \
                 but current bzlmod resolution facts data root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                data.workspace_id.canonical_project_root.display()
            ));
        }
        if data.resolution_digest != self.resolution_digest {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "BzlmodResolutionFactsKey was computed with resolution digest '{}', \
                 but current bzlmod resolution facts data digest is '{}'",
                self.resolution_digest,
                data.resolution_digest
            ));
        }
        Ok(data)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

#[async_trait]
impl Key for ModuleVersionsKey {
    type Value = slug_error::Result<Arc<ModuleVersionsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let data = ctx.compute(&BzlmodModuleVersionsDataKey).await?;
        if data.workspace_id != self.workspace_id {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "ModuleVersionsKey was computed with project root '{}', \
                 but current bzlmod module versions data root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                data.workspace_id.canonical_project_root.display()
            ));
        }
        if data.resolution_digest != self.resolution_digest {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "ModuleVersionsKey was computed with resolution digest '{}', \
                 but current bzlmod module versions data digest is '{}'",
                self.resolution_digest,
                data.resolution_digest
            ));
        }
        let lockfile_inputs = ctx
            .compute(&BzlmodLockfileInputsKey {
                workspace_id: self.workspace_id.clone(),
                resolution_digest: self.resolution_digest.clone(),
            })
            .await??;
        let repo_env = ctx
            .compute(&BzlmodRepoEnvKey {
                workspace_id: self.workspace_id.clone(),
                resolution_digest: self.resolution_digest.clone(),
            })
            .await??;
        let repo_mappings = ctx
            .compute(&BzlmodRepoMappingsKey {
                workspace_id: self.workspace_id.clone(),
                resolution_digest: self.resolution_digest.clone(),
            })
            .await??;
        let resolution_facts = ctx
            .compute(&BzlmodResolutionFactsKey {
                workspace_id: self.workspace_id.clone(),
                resolution_digest: self.resolution_digest.clone(),
            })
            .await??;
        Ok(Arc::new(ModuleVersionsValue {
            workspace_id: data.workspace_id.clone(),
            module_versions: data.module_versions.clone(),
            invalidation: Arc::new(BzlmodModuleVersionsInvalidation {
                root_module_name: data.root_module_name.clone(),
                lockfile_inputs,
                repo_env: repo_env.as_ref().clone(),
                registry_file_hashes: resolution_facts.registry_file_hashes.clone(),
                selected_yanked_versions: resolution_facts.selected_yanked_versions.clone(),
                repo_mappings: repo_mappings.repo_mappings.as_ref().clone(),
                repo_mapping_overrides: repo_mappings.repo_mapping_overrides.as_ref().clone(),
            }),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

fn root_aliases_from_repo_mappings(
    repo_mappings: &BzlmodRepoMappingsDataValue,
) -> Vec<BzlmodCellGraphAlias> {
    repo_mappings
        .repo_mappings
        .get("")
        .into_iter()
        .flat_map(|mapping| mapping.iter())
        .map(|(apparent_name, target_name)| BzlmodCellGraphAlias {
            apparent_name: apparent_name.clone(),
            target_name: target_name.clone(),
        })
        .collect()
}

fn merge_declared_root_aliases(
    mut aliases: Vec<BzlmodCellGraphAlias>,
    declared_aliases: &[BzlmodCellGraphAlias],
) -> Vec<BzlmodCellGraphAlias> {
    let mut seen: BTreeSet<_> = aliases
        .iter()
        .map(|alias| alias.apparent_name.clone())
        .collect();
    for alias in declared_aliases {
        if seen.insert(alias.apparent_name.clone()) {
            aliases.push(alias.clone());
        }
    }
    aliases
}

fn dynamic_aliases_from_repo_mappings(
    repo_mappings: &BzlmodRepoMappingsDataValue,
) -> Vec<BzlmodCellGraphDynamicAlias> {
    repo_mappings
        .repo_mappings
        .get("")
        .into_iter()
        .flat_map(|mapping| mapping.iter())
        .filter_map(|(apparent_name, target_name)| {
            if apparent_name != target_name
                && crate::pending_repo_cells::parse_canonical_name(apparent_name).is_some()
            {
                Some(BzlmodCellGraphDynamicAlias {
                    apparent_name: apparent_name.clone(),
                    canonical_name: target_name.clone(),
                })
            } else {
                None
            }
        })
        .collect()
}

fn merge_declared_dynamic_aliases(
    mut aliases: Vec<BzlmodCellGraphDynamicAlias>,
    declared_aliases: &[BzlmodCellGraphDynamicAlias],
) -> Vec<BzlmodCellGraphDynamicAlias> {
    let mut seen: BTreeSet<_> = aliases
        .iter()
        .map(|alias| alias.apparent_name.clone())
        .collect();
    for alias in declared_aliases {
        if seen.insert(alias.apparent_name.clone()) {
            aliases.push(alias.clone());
        }
    }
    aliases
}

fn scoped_aliases_from_repo_mappings(
    repo_mappings: &BzlmodRepoMappingsDataValue,
    root_module_name: &str,
) -> Vec<BzlmodCellGraphScopedAlias> {
    let mut scoped_aliases = Vec::new();
    for (owner_module, mapping) in repo_mappings.repo_mappings.iter() {
        if owner_module.is_empty() {
            continue;
        }
        for (apparent_name, target_name) in mapping {
            scoped_aliases.push(BzlmodCellGraphScopedAlias {
                owner_module: owner_module.clone(),
                apparent_name: apparent_name.clone(),
                target_name: target_name.clone(),
            });
        }
    }
    for (extension_id, overrides) in repo_mappings.repo_mapping_overrides.iter() {
        let owner_module =
            crate::extension_execution_dice::extract_owning_module(extension_id, root_module_name);
        for (apparent_name, target_name) in overrides {
            scoped_aliases.push(BzlmodCellGraphScopedAlias {
                owner_module: owner_module.clone(),
                apparent_name: apparent_name.clone(),
                target_name: target_name.clone(),
            });
            if let Some(owner_without_separator) = owner_module.strip_suffix('+') {
                scoped_aliases.push(BzlmodCellGraphScopedAlias {
                    owner_module: owner_without_separator.to_owned(),
                    apparent_name: apparent_name.clone(),
                    target_name: target_name.clone(),
                });
            }
        }
    }
    scoped_aliases
}

fn merge_declared_scoped_aliases(
    mut aliases: Vec<BzlmodCellGraphScopedAlias>,
    declared_aliases: &[BzlmodCellGraphScopedAlias],
) -> Vec<BzlmodCellGraphScopedAlias> {
    let mut seen: BTreeSet<_> = aliases
        .iter()
        .map(|alias| (alias.owner_module.clone(), alias.apparent_name.clone()))
        .collect();
    for alias in declared_aliases {
        if seen.insert((alias.owner_module.clone(), alias.apparent_name.clone())) {
            aliases.push(alias.clone());
        }
    }
    aliases
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "RegisteredToolchainsKey({}, {})",
    workspace_id.stable_hash(),
    resolution_digest
)]
pub struct RegisteredToolchainsKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

impl RegisteredToolchainsKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
        }
    }

    pub fn for_workspace_id_with_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct RegisteredToolchainsValue {
    pub workspace_id: WorkspaceId,
    pub registered_toolchains: Vec<crate::RegisteredToolchain>,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct RegisteredToolchainsDataValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub registered_toolchains: Vec<crate::RegisteredToolchain>,
}

impl RegisteredToolchainsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        registered_toolchains: Vec<crate::RegisteredToolchain>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            registered_toolchains,
        }
    }

    pub fn with_resolution_digest(mut self, resolution_digest: Arc<str>) -> Self {
        self.resolution_digest = resolution_digest;
        self
    }
}

#[derive(
    derive_more::Display,
    Debug,
    Hash,
    Eq,
    Clone,
    Dupe,
    PartialEq,
    Allocative
)]
#[display("BzlmodRegisteredToolchainsDataKey")]
pub struct BzlmodRegisteredToolchainsDataKey;

impl dice::InjectedKey for BzlmodRegisteredToolchainsDataKey {
    type Value = Arc<RegisteredToolchainsDataValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for RegisteredToolchainsKey {
    type Value = slug_error::Result<Arc<RegisteredToolchainsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let data = ctx.compute(&BzlmodRegisteredToolchainsDataKey).await?;
        if data.workspace_id != self.workspace_id {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "RegisteredToolchainsKey was computed with project root '{}', \
                 but current bzlmod registered toolchain data root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                data.workspace_id.canonical_project_root.display()
            ));
        }
        if data.resolution_digest != self.resolution_digest {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "RegisteredToolchainsKey was computed with resolution digest '{}', \
                 but current bzlmod registered toolchain data digest is '{}'",
                self.resolution_digest,
                data.resolution_digest
            ));
        }
        Ok(Arc::new(RegisteredToolchainsValue {
            workspace_id: self.workspace_id.clone(),
            registered_toolchains: data.registered_toolchains.clone(),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "RegisteredExecutionPlatformsKey({}, {})",
    workspace_id.stable_hash(),
    resolution_digest
)]
pub struct RegisteredExecutionPlatformsKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
}

impl RegisteredExecutionPlatformsKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
        }
    }

    pub fn for_workspace_id_with_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct RegisteredExecutionPlatformsValue {
    pub workspace_id: WorkspaceId,
    pub registered_execution_platforms: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct RegisteredExecutionPlatformsDataValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub registered_execution_platforms: Vec<String>,
}

impl RegisteredExecutionPlatformsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        registered_execution_platforms: Vec<String>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            registered_execution_platforms,
        }
    }

    pub fn with_resolution_digest(mut self, resolution_digest: Arc<str>) -> Self {
        self.resolution_digest = resolution_digest;
        self
    }
}

#[derive(
    derive_more::Display,
    Debug,
    Hash,
    Eq,
    Clone,
    Dupe,
    PartialEq,
    Allocative
)]
#[display("BzlmodRegisteredExecutionPlatformsDataKey")]
pub struct BzlmodRegisteredExecutionPlatformsDataKey;

impl dice::InjectedKey for BzlmodRegisteredExecutionPlatformsDataKey {
    type Value = Arc<RegisteredExecutionPlatformsDataValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for RegisteredExecutionPlatformsKey {
    type Value = slug_error::Result<Arc<RegisteredExecutionPlatformsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let data = ctx
            .compute(&BzlmodRegisteredExecutionPlatformsDataKey)
            .await?;
        if data.workspace_id != self.workspace_id {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "RegisteredExecutionPlatformsKey was computed with project root '{}', \
                 but current bzlmod registered execution platform data root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                data.workspace_id.canonical_project_root.display()
            ));
        }
        if data.resolution_digest != self.resolution_digest {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "RegisteredExecutionPlatformsKey was computed with resolution digest '{}', \
                 but current bzlmod registered execution platform data digest is '{}'",
                self.resolution_digest,
                data.resolution_digest
            ));
        }
        Ok(Arc::new(RegisteredExecutionPlatformsValue {
            workspace_id: self.workspace_id.clone(),
            registered_execution_platforms: data.registered_execution_platforms.clone(),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub enum RepoOriginKind {
    RootModule,
    Module(ModuleKey),
    ExtensionGenerated { extension: ModuleExtensionId },
    InnateRepoRule { owner_module: ModuleKey },
    Bundled,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleExtensionId {
    pub bzl_file_label: Arc<str>,
    pub extension_name: Arc<str>,
    pub isolation_key: Option<Arc<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ExtensionUniqueName {
    pub workspace_id: WorkspaceId,
    pub extension_instance_id: ModuleExtensionId,
    pub unique_name: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleExtensionAggregationKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub extension_instance_id: ModuleExtensionId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleExtensionReplayInputKey {
    pub workspace_id: WorkspaceId,
    pub extension_instance_id: ModuleExtensionId,
    pub lockfile_entry_digest: Arc<str>,
    pub bzl_transitive_digest: Arc<str>,
    pub usages_digest: Arc<str>,
    pub recorded_inputs_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ModuleExtensionExecutionIdentity {
    pub workspace_id: WorkspaceId,
    pub extension_instance_id: ModuleExtensionId,
    pub command_policy_digest: Arc<str>,
    pub replay_inputs_digest: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct InnateExtensionKey {
    pub workspace_id: WorkspaceId,
    pub owner_module_key: ModuleKey,
    pub bzl_label: Arc<str>,
    pub rule_name: Arc<str>,
    pub invocation_id: Arc<str>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "ExtensionBzlTransitiveDigestKey({}, {}, {}, allow_missing_loads={})",
    workspace_id.stable_hash(),
    extension_id,
    resolution_digest,
    allow_missing_loads
)]
pub struct ExtensionBzlTransitiveDigestKey {
    pub workspace_id: WorkspaceId,
    pub extension_id: Arc<str>,
    pub resolution_digest: Arc<str>,
    pub allow_missing_loads: bool,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "BzlmodExtensionAggregationKey({}, {}, {})",
    workspace_id.stable_hash(),
    resolution_digest,
    extension_id
)]
pub struct BzlmodExtensionAggregationKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub extension_id: Arc<str>,
}

impl BzlmodExtensionAggregationKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId, extension_id: &str) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            extension_id: Arc::from(extension_id),
        }
    }

    pub fn for_workspace_id_with_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
        extension_id: &str,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
            extension_id: Arc::from(extension_id),
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf, extension_id: &str) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root), extension_id)
    }
}

#[derive(Clone, Debug, Display, Allocative)]
#[display(
    "ExtensionSpokesKey({}, {}, {}, {}, {}, {})",
    workspace_id.stable_hash(),
    resolution_digest,
    extension_id,
    bzl_transitive_digest,
    usages_digest,
    replay_inputs_identity_digest
)]
pub struct ExtensionSpokesKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub extension_id: Arc<str>,
    pub bzl_transitive_digest: Arc<str>,
    pub usages_digest: Arc<str>,
    pub root_module_name: Arc<str>,
    #[allocative(skip)]
    pub aggregated: Arc<AggregatedExtension>,
    pub replay_inputs_identity_digest: Arc<str>,
    pub repo_env: Arc<BTreeMap<String, String>>,
    pub repo_mappings: Arc<crate::RepoMappingSnapshot>,
    pub repo_mapping_overrides: Arc<crate::RepoMappingOverrides>,
}

impl PartialEq for ExtensionSpokesKey {
    fn eq(&self, other: &Self) -> bool {
        self.workspace_id == other.workspace_id
            && self.resolution_digest == other.resolution_digest
            && self.extension_id == other.extension_id
            && self.bzl_transitive_digest == other.bzl_transitive_digest
            && self.usages_digest == other.usages_digest
            && self.root_module_name == other.root_module_name
            && self.replay_inputs_identity_digest == other.replay_inputs_identity_digest
            && self.repo_env == other.repo_env
            && self.repo_mappings == other.repo_mappings
            && self.repo_mapping_overrides == other.repo_mapping_overrides
    }
}

impl Eq for ExtensionSpokesKey {}

impl Hash for ExtensionSpokesKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.workspace_id.hash(state);
        self.resolution_digest.hash(state);
        self.extension_id.hash(state);
        self.bzl_transitive_digest.hash(state);
        self.usages_digest.hash(state);
        self.root_module_name.hash(state);
        self.replay_inputs_identity_digest.hash(state);
        self.repo_env.hash(state);
        self.repo_mappings.hash(state);
        self.repo_mapping_overrides.hash(state);
    }
}

impl ExtensionSpokesKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId, extension_id: &str) -> Self {
        Self::for_workspace_id_with_digest(workspace_id, extension_id, "")
    }

    #[cfg(test)]
    pub fn for_workspace_id_with_digest(
        workspace_id: WorkspaceId,
        extension_id: &str,
        bzl_transitive_digest: &str,
    ) -> Self {
        Self::for_workspace_id_with_inputs(
            workspace_id,
            extension_id,
            bzl_transitive_digest,
            "",
            "",
            "",
            Arc::new(AggregatedExtension::default()),
            Arc::new(BTreeMap::new()),
            Arc::new(crate::RepoMappingSnapshot::new()),
            Arc::new(crate::RepoMappingOverrides::new()),
        )
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub fn for_workspace_id_with_inputs(
        workspace_id: WorkspaceId,
        extension_id: &str,
        bzl_transitive_digest: &str,
        usages_digest: &str,
        root_module_name: &str,
        replay_inputs_identity_digest: &str,
        aggregated: Arc<AggregatedExtension>,
        repo_env: Arc<BTreeMap<String, String>>,
        repo_mappings: Arc<crate::RepoMappingSnapshot>,
        repo_mapping_overrides: Arc<crate::RepoMappingOverrides>,
    ) -> Self {
        Self::for_workspace_id_with_resolution_digest_and_inputs(
            workspace_id,
            Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            extension_id,
            bzl_transitive_digest,
            usages_digest,
            root_module_name,
            replay_inputs_identity_digest,
            aggregated,
            repo_env,
            repo_mappings,
            repo_mapping_overrides,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn for_workspace_id_with_resolution_digest_and_inputs(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
        extension_id: &str,
        bzl_transitive_digest: &str,
        usages_digest: &str,
        root_module_name: &str,
        replay_inputs_identity_digest: &str,
        aggregated: Arc<AggregatedExtension>,
        repo_env: Arc<BTreeMap<String, String>>,
        repo_mappings: Arc<crate::RepoMappingSnapshot>,
        repo_mapping_overrides: Arc<crate::RepoMappingOverrides>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
            extension_id: Arc::from(extension_id),
            bzl_transitive_digest: Arc::from(bzl_transitive_digest),
            usages_digest: Arc::from(usages_digest),
            root_module_name: Arc::from(root_module_name),
            replay_inputs_identity_digest: Arc::from(replay_inputs_identity_digest),
            aggregated,
            repo_env,
            repo_mappings,
            repo_mapping_overrides,
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf, extension_id: &str) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root), extension_id)
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "ExtensionSpokesByExtensionIdKey({}, {}, {})",
    workspace_id.stable_hash(),
    resolution_digest,
    extension_id
)]
pub struct ExtensionSpokesByExtensionIdKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub extension_id: Arc<str>,
}

impl ExtensionSpokesByExtensionIdKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId, extension_id: &str) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            extension_id: Arc::from(extension_id),
        }
    }

    pub fn for_workspace_id_with_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
        extension_id: &str,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
            extension_id: Arc::from(extension_id),
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf, extension_id: &str) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root), extension_id)
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "ExtensionIdByCanonicalRepoKey({}, {}, {})",
    workspace_id.stable_hash(),
    resolution_digest,
    canonical_name
)]
pub struct ExtensionIdByCanonicalRepoKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub canonical_name: Arc<str>,
}

impl ExtensionIdByCanonicalRepoKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId, canonical_name: &str) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            canonical_name: Arc::from(canonical_name),
        }
    }

    pub fn for_workspace_id_with_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
        canonical_name: &str,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
            canonical_name: Arc::from(canonical_name),
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf, canonical_name: &str) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root), canonical_name)
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "ExtensionSpokesByCanonicalRepoKey({}, {}, {})",
    workspace_id.stable_hash(),
    resolution_digest,
    canonical_name
)]
pub struct ExtensionSpokesByCanonicalRepoKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    pub canonical_name: Arc<str>,
}

impl ExtensionSpokesByCanonicalRepoKey {
    #[cfg(test)]
    pub fn for_workspace_id(workspace_id: WorkspaceId, canonical_name: &str) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
            canonical_name: Arc::from(canonical_name),
        }
    }

    pub fn for_workspace_id_with_resolution_digest(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
        canonical_name: &str,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
            canonical_name: Arc::from(canonical_name),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct ExtensionSpoke {
    pub internal_name: Arc<str>,
    pub canonical_name: Arc<str>,
    pub spec_hash: Arc<str>,
    pub repo_spec_json: Arc<str>,
    pub repo_spec: Arc<RepoSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct ExtensionSpokesValue {
    pub workspace_id: WorkspaceId,
    pub extension_id: Arc<str>,
    pub bzl_transitive_digest: Arc<str>,
    pub usages_digest: Arc<str>,
    pub replay_inputs_identity_digest: Arc<str>,
    pub repo_mappings_digest: Arc<str>,
    pub repo_mapping_overrides_digest: Arc<str>,
    pub project_root: Arc<PathBuf>,
    pub repo_env: Arc<BTreeMap<String, String>>,
    pub spokes: BTreeMap<String, ExtensionSpoke>,
    pub(crate) recorded_inputs: Arc<Vec<String>>,
    pub(crate) recorded_input_workspace_root: Option<Arc<PathBuf>>,
    pub(crate) recorded_input_repo_env: Arc<BTreeMap<String, String>>,
    pub(crate) recorded_input_repo_mappings: Arc<crate::RepoMappingSnapshot>,
}

impl ExtensionSpokesValue {
    pub fn by_internal_name(&self, internal_name: &str) -> Option<&ExtensionSpoke> {
        self.spokes.get(internal_name)
    }

    pub fn by_canonical_name(&self, canonical_name: &str) -> Option<&ExtensionSpoke> {
        self.spokes
            .values()
            .find(|spoke| spoke.canonical_name.as_ref() == canonical_name)
    }

    pub fn by_canonical_or_internal_name(&self, name: &str) -> Option<&ExtensionSpoke> {
        self.by_canonical_name(name)
            .or_else(|| self.by_internal_name(name))
    }

    pub fn iter(&self) -> impl Iterator<Item = &ExtensionSpoke> {
        self.spokes.values()
    }

    pub fn recorded_inputs(&self) -> &[String] {
        self.recorded_inputs.as_slice()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ExtensionRepoExecutionIdentity {
    pub workspace_id: WorkspaceId,
    pub canonical_repo: Arc<str>,
    pub repo_spec_digest: Arc<str>,
    pub repo_rule_impl_digest: Arc<str>,
    pub repo_replay_inputs_digest: Arc<str>,
}

#[derive(Clone, Debug, Display, Eq, Allocative)]
#[display(
    "RepoMaterializationManifestKey({}, {}, {})",
    workspace_id.stable_hash(),
    canonical_repo,
    repo_spec_digest
)]
pub struct RepoMaterializationManifestKey {
    pub workspace_id: WorkspaceId,
    pub output_base: Arc<PathBuf>,
    pub canonical_repo: Arc<str>,
    pub repo_spec_digest: Arc<str>,
    pub repo_spec: Arc<RepoSpec>,
    pub repo_env: Arc<BTreeMap<String, String>>,
    pub repo_mappings: Arc<crate::RepoMappingSnapshot>,
}

impl RepoMaterializationManifestKey {
    #[cfg(test)]
    pub fn for_workspace_id(
        workspace_id: WorkspaceId,
        canonical_repo: &str,
        repo_spec: Arc<RepoSpec>,
    ) -> Self {
        let repo_spec_digest = repo_spec.compute_hash();
        Self::for_workspace_id_with_repo_spec_digest(
            workspace_id,
            canonical_repo,
            repo_spec,
            repo_spec_digest,
        )
    }

    #[cfg(test)]
    pub fn for_project_root(
        project_root: PathBuf,
        canonical_repo: &str,
        repo_spec: Arc<RepoSpec>,
    ) -> Self {
        Self::for_workspace_id(
            WorkspaceId::for_project_root(project_root),
            canonical_repo,
            repo_spec,
        )
    }

    #[cfg(test)]
    fn for_workspace_id_with_repo_spec_digest(
        workspace_id: WorkspaceId,
        canonical_repo: &str,
        repo_spec: Arc<RepoSpec>,
        repo_spec_digest: String,
    ) -> Self {
        Self::for_workspace_id_with_repo_spec_digest_and_repo_env(
            workspace_id,
            canonical_repo,
            repo_spec,
            repo_spec_digest,
            Arc::new(BTreeMap::new()),
        )
    }

    #[cfg(test)]
    pub fn for_project_root_with_repo_spec_digest(
        project_root: PathBuf,
        canonical_repo: &str,
        repo_spec: Arc<RepoSpec>,
        repo_spec_digest: String,
    ) -> Self {
        Self::for_workspace_id_with_repo_spec_digest(
            WorkspaceId::for_project_root(project_root),
            canonical_repo,
            repo_spec,
            repo_spec_digest,
        )
    }

    #[cfg(test)]
    pub fn for_workspace_id_with_repo_spec_digest_and_repo_env(
        workspace_id: WorkspaceId,
        canonical_repo: &str,
        repo_spec: Arc<RepoSpec>,
        repo_spec_digest: String,
        repo_env: Arc<BTreeMap<String, String>>,
    ) -> Self {
        Self::for_workspace_id_with_repo_spec_digest_repo_env_and_repo_mappings(
            workspace_id,
            canonical_repo,
            repo_spec,
            repo_spec_digest,
            repo_env,
            Arc::new(crate::RepoMappingSnapshot::new()),
        )
    }

    pub fn for_workspace_id_with_repo_spec_digest_repo_env_and_repo_mappings(
        workspace_id: WorkspaceId,
        canonical_repo: &str,
        repo_spec: Arc<RepoSpec>,
        repo_spec_digest: String,
        repo_env: Arc<BTreeMap<String, String>>,
        repo_mappings: Arc<crate::RepoMappingSnapshot>,
    ) -> Self {
        Self {
            output_base: workspace_id.output_base.clone(),
            workspace_id,
            canonical_repo: Arc::from(canonical_repo),
            repo_spec_digest: Arc::from(repo_spec_digest.as_str()),
            repo_spec,
            repo_env,
            repo_mappings,
        }
    }

    #[cfg(test)]
    pub fn for_project_root_with_repo_spec_digest_and_repo_env(
        project_root: PathBuf,
        canonical_repo: &str,
        repo_spec: Arc<RepoSpec>,
        repo_spec_digest: String,
        repo_env: Arc<BTreeMap<String, String>>,
    ) -> Self {
        Self::for_workspace_id_with_repo_spec_digest_and_repo_env(
            WorkspaceId::for_project_root(project_root),
            canonical_repo,
            repo_spec,
            repo_spec_digest,
            repo_env,
        )
    }
}

impl PartialEq for RepoMaterializationManifestKey {
    fn eq(&self, other: &Self) -> bool {
        self.workspace_id == other.workspace_id
            && self.output_base == other.output_base
            && self.canonical_repo == other.canonical_repo
            && self.repo_spec_digest == other.repo_spec_digest
            && self.repo_env == other.repo_env
            && self.repo_mappings == other.repo_mappings
    }
}

impl std::hash::Hash for RepoMaterializationManifestKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.workspace_id.hash(state);
        self.output_base.hash(state);
        self.canonical_repo.hash(state);
        self.repo_spec_digest.hash(state);
        self.repo_env.hash(state);
        self.repo_mappings.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct RepoMaterializationManifestValue {
    pub key: RepoMaterializationManifestKey,
    pub repo_dir: Arc<PathBuf>,
    pub marker_state: Arc<str>,
    pub layout_state: Arc<str>,
    pub recorded_inputs_state: Arc<str>,
    pub output_tree_state: Arc<str>,
    pub digest: Arc<str>,
}

impl RepoMaterializationManifestValue {
    pub fn new(
        key: RepoMaterializationManifestKey,
        repo_dir: PathBuf,
        marker_state: String,
        layout_state: String,
        recorded_inputs_state: String,
        output_tree_state: String,
    ) -> Self {
        let digest = repo_materialization_manifest_digest(
            &key,
            &marker_state,
            &layout_state,
            &recorded_inputs_state,
            &output_tree_state,
        );
        Self {
            key,
            repo_dir: Arc::new(repo_dir),
            marker_state: Arc::from(marker_state.as_str()),
            layout_state: Arc::from(layout_state.as_str()),
            recorded_inputs_state: Arc::from(recorded_inputs_state.as_str()),
            output_tree_state: Arc::from(output_tree_state.as_str()),
            digest: Arc::from(digest.as_str()),
        }
    }

    pub fn state_summary(&self) -> String {
        format!(
            "{};{};{};{}",
            self.marker_state,
            self.layout_state,
            self.recorded_inputs_state,
            self.output_tree_state
        )
    }
}

fn repo_materialization_manifest_digest(
    key: &RepoMaterializationManifestKey,
    marker_state: &str,
    layout_state: &str,
    recorded_inputs_state: &str,
    output_tree_state: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"repo-materialization-manifest-v2");
    hasher.update([0]);
    update_digest_str(&mut hasher, key.workspace_id.stable_hash());
    update_digest_str(&mut hasher, &key.canonical_repo);
    update_digest_str(&mut hasher, &key.repo_spec_digest);
    update_digest_str(&mut hasher, marker_state);
    update_digest_str(&mut hasher, layout_state);
    update_digest_str(&mut hasher, recorded_inputs_state);
    update_digest_str(&mut hasher, output_tree_state);
    hex::encode(hasher.finalize())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct ExternalSymlinkLayoutKey {
    pub workspace_id: WorkspaceId,
    pub output_base: Arc<PathBuf>,
    pub cell_graph_digest: Arc<str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BzlmodEventKind {
    BzlmodResolutionCompute,
    ModuleFileParse,
    ExtensionEval,
    ExtensionReplayHit,
    ExtensionReplayMissReason,
    ExtensionSpokesCompute,
    RepoMaterializationStateRead,
    RepoMaterializationHit,
    RepoMaterializationMissReason,
    LockfileRead,
    LockfileWriteAttempt,
}

impl BzlmodEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BzlmodResolutionCompute => "bzlmod_resolution_compute",
            Self::ModuleFileParse => "module_file_parse",
            Self::ExtensionEval => "extension_eval",
            Self::ExtensionReplayHit => "extension_replay_hit",
            Self::ExtensionReplayMissReason => "extension_replay_miss_reason",
            Self::ExtensionSpokesCompute => "extension_spokes_compute",
            Self::RepoMaterializationStateRead => "repo_materialization_state_read",
            Self::RepoMaterializationHit => "repo_materialization_hit",
            Self::RepoMaterializationMissReason => "repo_materialization_miss_reason",
            Self::LockfileRead => "lockfile_read",
            Self::LockfileWriteAttempt => "lockfile_write_attempt",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct BzlmodEventCounters {
    pub bzlmod_resolution_compute: u64,
    pub module_file_parse: u64,
    pub extension_eval: u64,
    pub extension_replay_hit: u64,
    pub extension_replay_miss_reason: u64,
    pub extension_spokes_compute: u64,
    pub repo_materialization_state_read: u64,
    pub repo_materialization_hit: u64,
    pub repo_materialization_miss_reason: u64,
    pub lockfile_read: u64,
    pub lockfile_write_attempt: u64,
}

static BZLMOD_RESOLUTION_COMPUTE: AtomicU64 = AtomicU64::new(0);
static MODULE_FILE_PARSE: AtomicU64 = AtomicU64::new(0);
static EXTENSION_EVAL: AtomicU64 = AtomicU64::new(0);
static EXTENSION_REPLAY_HIT: AtomicU64 = AtomicU64::new(0);
static EXTENSION_REPLAY_MISS_REASON: AtomicU64 = AtomicU64::new(0);
static EXTENSION_SPOKES_COMPUTE: AtomicU64 = AtomicU64::new(0);
static REPO_MATERIALIZATION_STATE_READ: AtomicU64 = AtomicU64::new(0);
static REPO_MATERIALIZATION_HIT: AtomicU64 = AtomicU64::new(0);
static REPO_MATERIALIZATION_MISS_REASON: AtomicU64 = AtomicU64::new(0);
static LOCKFILE_READ: AtomicU64 = AtomicU64::new(0);
static LOCKFILE_WRITE_ATTEMPT: AtomicU64 = AtomicU64::new(0);

fn counter(kind: BzlmodEventKind) -> &'static AtomicU64 {
    match kind {
        BzlmodEventKind::BzlmodResolutionCompute => &BZLMOD_RESOLUTION_COMPUTE,
        BzlmodEventKind::ModuleFileParse => &MODULE_FILE_PARSE,
        BzlmodEventKind::ExtensionEval => &EXTENSION_EVAL,
        BzlmodEventKind::ExtensionReplayHit => &EXTENSION_REPLAY_HIT,
        BzlmodEventKind::ExtensionReplayMissReason => &EXTENSION_REPLAY_MISS_REASON,
        BzlmodEventKind::ExtensionSpokesCompute => &EXTENSION_SPOKES_COMPUTE,
        BzlmodEventKind::RepoMaterializationStateRead => &REPO_MATERIALIZATION_STATE_READ,
        BzlmodEventKind::RepoMaterializationHit => &REPO_MATERIALIZATION_HIT,
        BzlmodEventKind::RepoMaterializationMissReason => &REPO_MATERIALIZATION_MISS_REASON,
        BzlmodEventKind::LockfileRead => &LOCKFILE_READ,
        BzlmodEventKind::LockfileWriteAttempt => &LOCKFILE_WRITE_ATTEMPT,
    }
}

pub fn record_bzlmod_event(kind: BzlmodEventKind, detail: impl AsRef<str>) -> u64 {
    let count = counter(kind).fetch_add(1, Ordering::Relaxed) + 1;
    tracing::debug!(
        target: "slug_bzlmod::plan61",
        event_name = kind.as_str(),
        count,
        detail = detail.as_ref(),
        "bzlmod plan61 event"
    );
    count
}

pub fn bzlmod_event_counters() -> BzlmodEventCounters {
    BzlmodEventCounters {
        bzlmod_resolution_compute: BZLMOD_RESOLUTION_COMPUTE.load(Ordering::Relaxed),
        module_file_parse: MODULE_FILE_PARSE.load(Ordering::Relaxed),
        extension_eval: EXTENSION_EVAL.load(Ordering::Relaxed),
        extension_replay_hit: EXTENSION_REPLAY_HIT.load(Ordering::Relaxed),
        extension_replay_miss_reason: EXTENSION_REPLAY_MISS_REASON.load(Ordering::Relaxed),
        extension_spokes_compute: EXTENSION_SPOKES_COMPUTE.load(Ordering::Relaxed),
        repo_materialization_state_read: REPO_MATERIALIZATION_STATE_READ.load(Ordering::Relaxed),
        repo_materialization_hit: REPO_MATERIALIZATION_HIT.load(Ordering::Relaxed),
        repo_materialization_miss_reason: REPO_MATERIALIZATION_MISS_REASON.load(Ordering::Relaxed),
        lockfile_read: LOCKFILE_READ.load(Ordering::Relaxed),
        lockfile_write_attempt: LOCKFILE_WRITE_ATTEMPT.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use async_trait::async_trait;

    use super::*;
    use crate::types::BazelDep;
    use crate::types::ExtensionUsage;
    use crate::types::GitOverride;
    use crate::types::LocalPathOverride;
    use crate::types::RegisteredItem;
    use crate::version::Version;

    #[test]
    fn workspace_id_distinguishes_project_roots() {
        let first = WorkspaceId::new(PathBuf::from("/tmp/ws-a"), PathBuf::from("/tmp/out"));
        let second = WorkspaceId::new(PathBuf::from("/tmp/ws-b"), PathBuf::from("/tmp/out"));

        assert_ne!(first, second);
        assert_ne!(first.stable_hash(), second.stable_hash());
    }

    #[test]
    fn workspace_id_names_no_project_sentinel() {
        let sentinel = WorkspaceId::no_project_sentinel();

        assert_eq!(sentinel.canonical_project_root.as_ref(), &PathBuf::new());
        assert_eq!(sentinel.output_base.as_ref(), &PathBuf::from("buck-out/v2"));
    }

    fn parsed_module(name: &str) -> ParsedModuleFile {
        ParsedModuleFile {
            module: Module::new(name.to_owned(), Version::empty()),
            has_module_directive: true,
            extension_usages: Vec::new(),
            repo_rule_invocations: Vec::new(),
            registered_toolchains: Vec::new(),
            registered_execution_platforms: Vec::new(),
        }
    }

    struct TestCleanGraphIo;

    static TEST_CLEAN_GRAPH_IO: TestCleanGraphIo = TestCleanGraphIo;
    static INIT_TEST_CLEAN_GRAPH_IO: Once = Once::new();
    static TEST_LOCKFILE_VALUES: OnceLock<Mutex<HashMap<PathBuf, Arc<LockfileContentValue>>>> =
        OnceLock::new();

    fn init_test_clean_graph_io() {
        INIT_TEST_CLEAN_GRAPH_IO.call_once(|| {
            BZLMOD_CLEAN_GRAPH_IO_IMPL.init(&TEST_CLEAN_GRAPH_IO);
        });
    }

    fn test_lockfile_values() -> &'static Mutex<HashMap<PathBuf, Arc<LockfileContentValue>>> {
        TEST_LOCKFILE_VALUES.get_or_init(|| Mutex::new(HashMap::new()))
    }

    #[async_trait]
    impl BzlmodCleanGraphIo for TestCleanGraphIo {
        async fn compute_source_inputs(
            &self,
            _key: &BzlmodResolvedModuleGraphKey,
            _ctx: &mut DiceComputations<'_>,
        ) -> slug_error::Result<BzlmodResolvedGraphSourceInputsValue> {
            Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "test clean graph IO does not provide source inputs"
            ))
        }

        async fn compute_non_root_module_files(
            &self,
            _key: &BzlmodResolvedModuleGraphKey,
            _ctx: &mut DiceComputations<'_>,
            _inputs: Vec<NonRootModuleFileInput>,
            _root_module_name: &str,
        ) -> slug_error::Result<Arc<NonRootModuleFilesValue>> {
            Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "test clean graph IO does not provide non-root module files"
            ))
        }

        async fn compute_lockfile_content(
            &self,
            _workspace_id: &WorkspaceId,
            _kind: LockfileContentKind,
            path: Arc<PathBuf>,
            _ctx: &mut DiceComputations<'_>,
        ) -> slug_error::Result<Arc<LockfileContentValue>> {
            let values = test_lockfile_values()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            Ok(values.get(path.as_ref()).cloned().unwrap_or_else(|| {
                Arc::new(LockfileContentValue {
                    path,
                    digest: None,
                    tracked_by_dice: false,
                    lockfile: None,
                })
            }))
        }
    }

    fn test_lockfile_value(path: PathBuf, digest: &str) -> Arc<LockfileContentValue> {
        Arc::new(LockfileContentValue {
            path: Arc::new(path),
            digest: Some(digest.to_owned()),
            tracked_by_dice: true,
            lockfile: None,
        })
    }

    fn test_polled_lockfile_value(path: PathBuf, digest: &str) -> Arc<LockfileContentValue> {
        Arc::new(LockfileContentValue {
            path: Arc::new(path),
            digest: Some(digest.to_owned()),
            tracked_by_dice: false,
            lockfile: None,
        })
    }

    #[tokio::test]
    async fn clean_lockfile_inputs_key_owns_mode_and_paths() -> slug_error::Result<()> {
        init_test_clean_graph_io();
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-bzlmod-clean-lockfile-ws"),
            PathBuf::from("/tmp/slug-bzlmod-clean-lockfile-out"),
        );
        let visible_path = lockfile_path(workspace_id.canonical_project_root.as_ref());
        let hidden_path = PathBuf::from("/tmp/slug-bzlmod-clean-hidden.lock");
        {
            let mut values = test_lockfile_values()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            values.insert(
                visible_path.clone(),
                test_lockfile_value(visible_path, "visible-digest"),
            );
            values.insert(
                hidden_path.clone(),
                test_lockfile_value(hidden_path.clone(), "hidden-digest"),
            );
        }

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut dice = dice;
        let inputs = dice
            .compute(&BzlmodCleanLockfileInputsKey {
                workspace_id: workspace_id.clone(),
                lockfile_mode: LockfileMode::Update,
                hidden_lockfile_path: Some(hidden_path.clone()),
                root_module_present: true,
            })
            .await??;
        assert_eq!(
            inputs.visible_lockfile_digest.as_deref(),
            Some("visible-digest")
        );
        assert_eq!(
            inputs.hidden_lockfile_digest.as_deref(),
            Some("hidden-digest")
        );

        let off_inputs = dice
            .compute(&BzlmodCleanLockfileInputsKey {
                workspace_id,
                lockfile_mode: LockfileMode::Off,
                hidden_lockfile_path: Some(hidden_path),
                root_module_present: true,
            })
            .await??;
        assert!(off_inputs.visible_lockfile.is_none());
        assert!(off_inputs.hidden_lockfile.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn lockfile_inputs_key_recomputes_from_policy_data() -> slug_error::Result<()> {
        init_test_clean_graph_io();
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-bzlmod-lockfile-inputs-ws"),
            PathBuf::from("/tmp/slug-bzlmod-lockfile-inputs-out"),
        );
        let visible_path = lockfile_path(workspace_id.canonical_project_root.as_ref());
        let hidden_path = PathBuf::from("/tmp/slug-bzlmod-lockfile-inputs-hidden.lock");
        {
            let mut values = test_lockfile_values()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            values.insert(
                visible_path.clone(),
                test_lockfile_value(visible_path, "visible-current"),
            );
            values.insert(
                hidden_path.clone(),
                test_lockfile_value(hidden_path.clone(), "hidden-current"),
            );
        }

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace_policy(
                workspace_id.clone(),
                LockfileMode::Update,
                Some(hidden_path),
                true,
            )),
        )])?;
        let mut dice = updater.commit().await;
        let inputs = dice
            .compute(&BzlmodLockfileInputsKey::for_workspace_id(workspace_id))
            .await??;

        assert_eq!(
            inputs.visible_lockfile_digest.as_deref(),
            Some("visible-current")
        );
        assert_eq!(
            inputs.hidden_lockfile_digest.as_deref(),
            Some("hidden-current")
        );
        Ok(())
    }

    #[tokio::test]
    async fn lockfile_inputs_key_rechecks_untracked_hidden_lockfile() -> slug_error::Result<()> {
        init_test_clean_graph_io();
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-bzlmod-lockfile-untracked-ws"),
            PathBuf::from("/tmp/slug-bzlmod-lockfile-untracked-out"),
        );
        let visible_path = lockfile_path(workspace_id.canonical_project_root.as_ref());
        let hidden_path = PathBuf::from("/tmp/slug-bzlmod-lockfile-untracked-hidden.lock");
        {
            let mut values = test_lockfile_values()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            values.insert(
                visible_path.clone(),
                test_lockfile_value(visible_path, "visible-current"),
            );
            values.insert(
                hidden_path.clone(),
                test_polled_lockfile_value(hidden_path.clone(), "hidden-first"),
            );
        }

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace_policy(
                workspace_id.clone(),
                LockfileMode::Update,
                Some(hidden_path.clone()),
                true,
            )),
        )])?;
        let mut dice = updater.commit().await;
        let first = dice
            .compute(&BzlmodLockfileInputsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        assert_eq!(
            first.hidden_lockfile_digest.as_deref(),
            Some("hidden-first")
        );
        assert!(!<BzlmodLockfileInputsKey as Key>::validity(&Ok(
            first.clone()
        )));

        {
            let mut values = test_lockfile_values()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            values.insert(
                hidden_path.clone(),
                test_polled_lockfile_value(hidden_path, "hidden-second"),
            );
        }
        let mut dice = dice.into_updater().commit().await;
        let second = dice
            .compute(&BzlmodLockfileInputsKey::for_workspace_id(workspace_id))
            .await??;
        assert_eq!(
            second.hidden_lockfile_digest.as_deref(),
            Some("hidden-second")
        );
        Ok(())
    }

    #[derive(Hash)]
    struct TestSourceInputsKey {
        name: &'static str,
    }

    fn resolved_graph_source_inputs_for_test(
        root_digest: Option<&str>,
        hidden_lockfile_digest: Option<&str>,
        local_digest: &str,
        registry_digest: &str,
        patch_digest: &str,
    ) -> BzlmodResolvedGraphSourceInputsValue {
        BzlmodResolvedGraphSourceInputsValue {
            root_module_file: Arc::new(RootModuleFileValue {
                path: Arc::new(PathBuf::from("/tmp/workspace/MODULE.bazel")),
                input_digest: root_digest.map(str::to_owned),
                input_count: usize::from(root_digest.is_some()),
                parsed: root_digest.map(|_| parsed_module("root")),
            }),
            lockfile_inputs: Arc::new(BzlmodLockfileInputsValue {
                hidden_lockfile_path: None,
                visible_lockfile_digest: None,
                hidden_lockfile_digest: hidden_lockfile_digest.map(str::to_owned),
                visible_lockfile: None,
                hidden_lockfile: None,
                lockfile_mode: LockfileMode::Update,
            }),
            local_override_inputs: Arc::new(LocalOverrideModuleInputsValue {
                digest: local_digest.to_owned(),
                parsed_modules: Vec::new(),
                missing_module_dirs: Vec::new(),
                has_bazel_deps: false,
                has_extension_usages: false,
                has_repo_rule_invocations: false,
                has_git_overrides: false,
                has_untracked_inputs: false,
            }),
            non_registry_override_inputs: Arc::new(NonRegistryOverrideModuleInputsValue {
                digest: "non-registry".to_owned(),
                parsed_modules: Vec::new(),
                module_dirs: Vec::new(),
                has_inputs: false,
                has_untracked_inputs: false,
            }),
            registry_file_inputs: Arc::new(RegistryFileInputsValue {
                digest: registry_digest.to_owned(),
                has_inputs: !registry_digest.is_empty(),
                cache_safe: true,
                has_untracked_inputs: false,
            }),
            override_patch_inputs: Arc::new(crate::OverridePatchInputs {
                digest: patch_digest.to_owned(),
                inputs: Vec::new(),
                has_untracked_inputs: false,
            }),
        }
    }

    #[test]
    fn resolved_graph_source_inputs_identity_digest_tracks_source_components() {
        let key = TestSourceInputsKey { name: "same-key" };
        let first = resolved_graph_source_inputs_for_test(
            Some("root-a"),
            None,
            "local-a",
            "reg-a",
            "patch-a",
        );
        let same = resolved_graph_source_inputs_for_test(
            Some("root-a"),
            None,
            "local-a",
            "reg-a",
            "patch-a",
        );

        assert_eq!(
            first.identity_digest_with_key(&key),
            same.identity_digest_with_key(&key)
        );
        assert_ne!(
            first.identity_digest_with_key(&key),
            resolved_graph_source_inputs_for_test(
                Some("root-b"),
                None,
                "local-a",
                "reg-a",
                "patch-a"
            )
            .identity_digest_with_key(&key)
        );
        assert_ne!(
            first.identity_digest_with_key(&key),
            resolved_graph_source_inputs_for_test(
                Some("root-a"),
                Some("hidden-lock"),
                "local-a",
                "reg-a",
                "patch-a"
            )
            .identity_digest_with_key(&key)
        );
        assert_ne!(
            first.identity_digest_with_key(&key),
            resolved_graph_source_inputs_for_test(
                Some("root-a"),
                None,
                "local-b",
                "reg-a",
                "patch-a"
            )
            .identity_digest_with_key(&key)
        );
        assert_ne!(
            first.identity_digest_with_key(&key),
            resolved_graph_source_inputs_for_test(
                Some("root-a"),
                None,
                "local-a",
                "reg-b",
                "patch-a"
            )
            .identity_digest_with_key(&key)
        );
        assert_ne!(
            first.identity_digest_with_key(&key),
            resolved_graph_source_inputs_for_test(
                Some("root-a"),
                None,
                "local-a",
                "reg-a",
                "patch-b"
            )
            .identity_digest_with_key(&key)
        );
    }

    #[test]
    fn resolved_graph_source_inputs_identity_digest_tracks_key_policy() {
        let inputs = resolved_graph_source_inputs_for_test(
            Some("root-a"),
            None,
            "local-a",
            "reg-a",
            "patch-a",
        );

        assert_ne!(
            inputs.identity_digest_with_key(&TestSourceInputsKey { name: "first" }),
            inputs.identity_digest_with_key(&TestSourceInputsKey { name: "second" })
        );
    }

    #[test]
    fn collect_registered_items_honors_ignore_dev_dependency_for_root() {
        let mut root = parsed_module("root");
        root.registered_toolchains.push(RegisteredItem {
            label: "@root_toolchains//:all".to_owned(),
            dev_dependency: true,
        });
        root.registered_execution_platforms.push(RegisteredItem {
            label: "@root_platforms//:all".to_owned(),
            dev_dependency: true,
        });

        let parsed_modules = vec![("root".to_owned(), root)];
        let (toolchains, platforms) =
            collect_bzlmod_registered_items(&parsed_modules, "root", false);
        assert_eq!(toolchains.len(), 1);
        assert_eq!(platforms.len(), 1);

        let (toolchains, platforms) =
            collect_bzlmod_registered_items(&parsed_modules, "root", true);
        assert!(toolchains.is_empty());
        assert!(platforms.is_empty());
    }

    #[test]
    fn collect_registered_items_auto_injects_rules_python_toolchains() {
        let parsed_modules = vec![("rules_python".to_owned(), parsed_module("rules_python"))];

        let (toolchains, platforms) =
            collect_bzlmod_registered_items(&parsed_modules, "root", false);

        assert!(platforms.is_empty());
        assert_eq!(
            toolchains
                .iter()
                .map(|toolchain| toolchain.label.as_str())
                .collect::<Vec<_>>(),
            vec![
                "@local_config_python//:host_toolchain",
                "@local_config_python//:host_launcher_maker_toolchain",
            ]
        );
    }

    #[test]
    fn collect_registered_items_skips_duplicate_rules_python_toolchains() {
        let mut rules_python = parsed_module("rules_python");
        rules_python.registered_toolchains.push(RegisteredItem {
            label: "@local_config_python//:custom".to_owned(),
            dev_dependency: false,
        });
        let parsed_modules = vec![("rules_python".to_owned(), rules_python)];

        let (toolchains, _platforms) =
            collect_bzlmod_registered_items(&parsed_modules, "root", false);

        assert_eq!(toolchains.len(), 1);
        assert_eq!(toolchains[0].label, "@local_config_python//:custom");
    }

    #[tokio::test]
    async fn resolve_graph_with_module_file_inputs_uses_tracked_local_overrides() {
        let mut root = parsed_module("root");
        root.module.bazel_deps.push(BazelDep::new(
            "dep".to_owned(),
            Version::parse("1.0").unwrap(),
        ));
        root.module
            .overrides
            .push(Override::LocalPath(LocalPathOverride {
                module_name: "dep".to_owned(),
                path: "dep".to_owned(),
            }));
        let mut dep = parsed_module("dep");
        dep.module.version = Version::parse("1.0").unwrap();
        let local_override_inputs = LocalOverrideModuleInputsValue {
            digest: "local".to_owned(),
            parsed_modules: vec![("dep".to_owned(), dep.clone())],
            missing_module_dirs: Vec::new(),
            has_bazel_deps: false,
            has_extension_usages: false,
            has_repo_rule_invocations: false,
            has_git_overrides: false,
            has_untracked_inputs: false,
        };
        let options = BzlmodResolutionOptions {
            lockfile_mode: LockfileMode::Off,
            ignore_dev_dependency: false,
            allow_yanked_versions_env: None,
            allow_yanked_versions_flags: Vec::new(),
            hidden_lockfile_path: None,
            repo_env: std::collections::BTreeMap::new(),
            repo_env_digest: "repo-env".to_owned(),
        };

        let resolved = resolve_graph_with_module_file_inputs(
            &root,
            Path::new("/workspace"),
            &options,
            &local_override_inputs,
            &NonRegistryOverrideModuleInputsValue {
                digest: "non-registry".to_owned(),
                parsed_modules: Vec::new(),
                module_dirs: Vec::new(),
                has_inputs: false,
                has_untracked_inputs: false,
            },
            Arc::new(crate::OverridePatchInputs::default()),
            None,
        )
        .await
        .unwrap();

        assert!(resolved.non_root_module_file_inputs.is_empty());
        assert_eq!(
            resolved
                .parsed_modules
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["root", "dep"]
        );
        let dep_info = resolved.graph.modules.get("dep").unwrap();
        assert_eq!(dep_info.source_path, Some(PathBuf::from("dep")));
        assert!(matches!(
            dep_info.source,
            ModuleSource::LocalPath { ref path } if path == "dep"
        ));
    }

    #[test]
    fn non_registry_override_inputs_include_patch_digest() {
        let mut root = parsed_module("root");
        let git = GitOverride {
            module_name: "dep".to_owned(),
            remote: "https://example.invalid/dep.git".to_owned(),
            commit: "abcdef".to_owned(),
            shallow_since: None,
            patches: vec!["//:fix.patch".to_owned()],
            patch_strip: 1,
        };
        root.module.overrides.push(Override::Git(git.clone()));
        let root_value = RootModuleFileValue {
            path: Arc::new(PathBuf::from("/tmp/workspace/MODULE.bazel")),
            input_digest: Some("root".to_owned()),
            input_count: 1,
            parsed: Some(root),
        };
        let patch_inputs = crate::OverridePatchInputs {
            digest: "patches".to_owned(),
            inputs: vec![crate::OverridePatchInput {
                label: "//:fix.patch".to_owned(),
                path: PathBuf::from("/tmp/workspace/fix.patch"),
                digest: "patch".to_owned(),
                content: b"diff --git a/MODULE.bazel b/MODULE.bazel\n".to_vec(),
            }],
            has_untracked_inputs: false,
        };
        let expected_patch_digest =
            crate::fetch::SourceFetcher::local_override_patch_digest_with_inputs(
                &git.patches,
                git.patch_strip,
                &patch_inputs,
            )
            .unwrap();
        let cache = ModuleCache::new().unwrap();

        let inputs =
            non_registry_override_module_inputs_from_root_module(&root_value, false, &patch_inputs)
                .unwrap();

        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].module_name, "dep");
        assert_eq!(
            inputs[0].module_dir,
            cache.git_override_dir_with_patch_digest(&git, expected_patch_digest.as_deref())
        );
        assert!(matches!(
            inputs[0].source,
            NonRegistryOverrideModuleSource::Git {
                ref remote,
                ref commit,
                patch_strip,
                ..
            } if remote == &git.remote && commit == &git.commit && patch_strip == git.patch_strip
        ));
    }

    #[test]
    fn resolved_graph_projection_values_collects_versions_and_facts() {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/output-base"),
        );
        let root = parsed_module("root");
        let mut graph = ResolvedGraph::default();
        graph.modules.insert(
            "dep".to_owned(),
            ResolvedModuleInfo {
                name: "dep".to_owned(),
                version: "1.0".to_owned(),
                compatibility_level: 0,
                dependencies: HashMap::new(),
                source: ModuleSource::Registry {
                    url: "https://registry.example".to_owned(),
                },
                source_path: Some(PathBuf::from("/tmp/dep")),
            },
        );
        graph.registry_file_hashes.insert(
            "https://registry.example/modules/dep/1.0/MODULE.bazel".to_owned(),
            "sha256".to_owned(),
        );
        let parsed_modules = vec![("root".to_owned(), root.clone())];

        let projections =
            resolved_graph_projection_values(workspace_id, &root, &parsed_modules, &graph, false);

        assert_eq!(projections.module_versions.root_module_name, "root");
        assert_eq!(
            projections
                .module_versions
                .module_versions
                .get("dep")
                .map(String::as_str),
            Some("1.0")
        );
        assert_eq!(
            projections
                .resolution_facts
                .registry_file_hashes
                .get("https://registry.example/modules/dep/1.0/MODULE.bazel")
                .map(String::as_str),
            Some("sha256")
        );
    }

    #[test]
    fn clean_resolved_graph_outputs_value_packages_projection_and_graph_digest() {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/project"),
            PathBuf::from("/tmp/output-base"),
        );
        let root = parsed_module("root");
        let mut graph = ResolvedGraph::default();
        graph.modules.insert(
            "dep".to_owned(),
            ResolvedModuleInfo {
                name: "dep".to_owned(),
                version: "1.0".to_owned(),
                compatibility_level: 0,
                dependencies: HashMap::new(),
                source: ModuleSource::Registry {
                    url: "https://registry.example".to_owned(),
                },
                source_path: Some(PathBuf::from("/tmp/dep")),
            },
        );
        let expected_digest = bzlmod_resolved_graph_digest(&graph);
        let parsed_modules = vec![("root".to_owned(), root.clone())];
        let cell_graph = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());

        let outputs = clean_resolved_graph_outputs_value(
            workspace_id.clone(),
            Arc::from("test-cell-graph-resolution-digest"),
            &root,
            &parsed_modules,
            graph,
            false,
            cell_graph,
        );

        assert_eq!(outputs.graph_digest.as_ref(), expected_digest.as_str());
        assert_eq!(
            outputs.cell_graph_resolution_digest.as_ref(),
            "test-cell-graph-resolution-digest"
        );
        assert_eq!(outputs.module_versions.workspace_id, workspace_id);
        assert_eq!(
            outputs
                .module_versions
                .module_versions
                .get("dep")
                .map(String::as_str),
            Some("1.0")
        );
        assert!(outputs.graph.modules.contains_key("dep"));
    }

    #[test]
    fn selected_bzlmod_cell_name_for_dep_prefers_canonical_module_repo() {
        let cell_names = vec!["dep+"];
        let mut resolved_graph = ResolvedGraph::default();
        resolved_graph.modules.insert(
            "dep".to_owned(),
            ResolvedModuleInfo {
                name: "dep".to_owned(),
                version: "1.0".to_owned(),
                compatibility_level: 0,
                dependencies: HashMap::new(),
                source: ModuleSource::Registry {
                    url: "https://bcr.bazel.build".to_owned(),
                },
                source_path: None,
            },
        );

        assert_eq!(
            selected_bzlmod_cell_name_for_dep(&cell_names, "dep", &resolved_graph),
            Some("dep+")
        );
    }

    #[test]
    fn repo_mapping_snapshot_targets_use_canonical_module_cells() {
        let cell_names = vec!["dep+", "other+"];
        let mut resolved_graph = ResolvedGraph::default();
        for module_name in ["dep", "other"] {
            resolved_graph.modules.insert(
                module_name.to_owned(),
                ResolvedModuleInfo {
                    name: module_name.to_owned(),
                    version: "1.0".to_owned(),
                    compatibility_level: 0,
                    dependencies: HashMap::new(),
                    source: ModuleSource::Registry {
                        url: "https://bcr.bazel.build".to_owned(),
                    },
                    source_path: None,
                },
            );
        }
        let mut snapshot = crate::RepoMappingSnapshot::new();
        snapshot.insert(
            "root".to_owned(),
            BTreeMap::from([
                ("dep".to_owned(), "dep".to_owned()),
                ("already_canonical".to_owned(), "other+".to_owned()),
            ]),
        );

        canonicalize_repo_mapping_snapshot_targets(
            &mut snapshot,
            &cell_names,
            Some(&resolved_graph),
        );

        let mapping = snapshot.get("root").expect("root mapping should exist");
        assert_eq!(mapping.get("dep").map(String::as_str), Some("dep+"));
        assert_eq!(
            mapping.get("already_canonical").map(String::as_str),
            Some("other+")
        );
    }

    #[test]
    fn repo_mapping_override_targets_use_canonical_root_mapping_targets() {
        let cell_names = vec!["dep+"];
        let mut resolved_graph = ResolvedGraph::default();
        resolved_graph.modules.insert(
            "dep".to_owned(),
            ResolvedModuleInfo {
                name: "dep".to_owned(),
                version: "1.0".to_owned(),
                compatibility_level: 0,
                dependencies: HashMap::new(),
                source: ModuleSource::Registry {
                    url: "https://bcr.bazel.build".to_owned(),
                },
                source_path: None,
            },
        );
        let mut snapshot = crate::RepoMappingSnapshot::new();
        snapshot.insert(
            String::new(),
            BTreeMap::from([
                ("helper_alias".to_owned(), "dep+".to_owned()),
                ("_main+ext+generated".to_owned(), "helper_alias".to_owned()),
            ]),
        );
        snapshot.insert("root".to_owned(), snapshot[""].clone());
        let extension_id = crate::canonical_extension_id("//:ext.bzl", "ext", "root");
        let generated_repo = "root+ext+generated".to_owned();
        let mut overrides = crate::RepoMappingOverrides::new();
        overrides.insert(
            extension_id.clone(),
            BTreeMap::from([("generated".to_owned(), "helper_alias".to_owned())]),
        );

        canonicalize_repo_mapping_snapshot_targets(
            &mut snapshot,
            &cell_names,
            Some(&resolved_graph),
        );
        canonicalize_repo_mapping_overrides_targets(
            &mut overrides,
            &snapshot,
            &cell_names,
            Some(&resolved_graph),
        );
        assert_eq!(
            snapshot
                .get("")
                .and_then(|mapping| mapping.get("_main+ext+generated"))
                .map(String::as_str),
            Some("dep+")
        );
        assert_eq!(
            overrides
                .get(&extension_id)
                .and_then(|mapping| mapping.get("generated"))
                .map(String::as_str),
            Some("dep+")
        );

        assert!(crate::add_extension_generated_repo_mappings(
            &mut snapshot,
            &extension_id,
            "root",
            [("generated".to_owned(), generated_repo.clone())],
            overrides.get(&extension_id),
        ));
        assert_eq!(
            snapshot
                .get(&generated_repo)
                .and_then(|mapping| mapping.get("generated"))
                .map(String::as_str),
            Some("dep+")
        );
    }

    #[test]
    fn graph_owned_repo_mapping_state_removes_root_apparent_override_targets() {
        let mut root = parsed_module("root");
        let mut dep = BazelDep::new("dep".to_owned(), Version::empty());
        dep.repo_name = Some("helper_alias".to_owned());
        root.module.bazel_deps.push(dep);
        let mut usage = ExtensionUsage::new("//:ext.bzl".to_owned(), "ext".to_owned());
        usage
            .repo_overrides
            .push(("generated".to_owned(), "helper_alias".to_owned()));
        root.extension_usages.push(usage);

        let (snapshot, overrides) =
            graph_owned_repo_mapping_state(&[("root".to_owned(), root)], "root", false, &[], None);
        let extension_id = crate::canonical_extension_id("//:ext.bzl", "ext", "root");

        assert_eq!(
            snapshot
                .get("")
                .and_then(|mapping| mapping.get("_main+ext+generated"))
                .map(String::as_str),
            Some("dep")
        );
        assert_eq!(
            overrides
                .get(&extension_id)
                .and_then(|mapping| mapping.get("generated"))
                .map(String::as_str),
            Some("dep")
        );
    }

    #[test]
    fn module_versions_key_equality_tracks_versions_value_and_invalidation() {
        let workspace_id = WorkspaceId::new(PathBuf::from("/tmp/ws"), PathBuf::from("/tmp/out"));
        let mut first_versions = HashMap::new();
        first_versions.insert("root".to_owned(), "1.0.0".to_owned());

        let mut second_versions = first_versions.clone();
        second_versions.insert("dep".to_owned(), "2.0.0".to_owned());

        let first = Ok(Arc::new(ModuleVersionsValue {
            workspace_id: workspace_id.clone(),
            module_versions: Arc::new(first_versions.clone()),
            invalidation: module_versions_invalidation(Some("first")),
        }));
        let same = Ok(Arc::new(ModuleVersionsValue {
            workspace_id: workspace_id.clone(),
            module_versions: Arc::new(first_versions),
            invalidation: module_versions_invalidation(Some("first")),
        }));
        let changed = Ok(Arc::new(ModuleVersionsValue {
            workspace_id: workspace_id.clone(),
            module_versions: Arc::new(second_versions),
            invalidation: module_versions_invalidation(Some("first")),
        }));
        let invalidated = Ok(Arc::new(ModuleVersionsValue {
            workspace_id,
            module_versions: same.as_ref().unwrap().module_versions.clone(),
            invalidation: module_versions_invalidation(Some("second")),
        }));

        assert!(<ModuleVersionsKey as Key>::equality(&first, &same));
        assert!(!<ModuleVersionsKey as Key>::equality(&first, &changed));
        assert!(!<ModuleVersionsKey as Key>::equality(&first, &invalidated));
    }

    fn module_versions_invalidation(
        hidden_lockfile_digest: Option<&str>,
    ) -> Arc<BzlmodModuleVersionsInvalidation> {
        Arc::new(BzlmodModuleVersionsInvalidation {
            root_module_name: "root".to_owned(),
            lockfile_inputs: Arc::new(BzlmodLockfileInputsValue {
                hidden_lockfile_path: None,
                visible_lockfile_digest: None,
                hidden_lockfile_digest: hidden_lockfile_digest.map(str::to_owned),
                visible_lockfile: None,
                hidden_lockfile: None,
                lockfile_mode: crate::LockfileMode::Update,
            }),
            repo_env: BTreeMap::new(),
            registry_file_hashes: indexmap::IndexMap::new(),
            selected_yanked_versions: indexmap::IndexMap::new(),
            repo_mappings: crate::RepoMappingSnapshot::new(),
            repo_mapping_overrides: crate::RepoMappingOverrides::new(),
        })
    }

    #[test]
    fn event_counters_are_addressable_by_plan61_names() {
        let before = bzlmod_event_counters();
        let count = record_bzlmod_event(BzlmodEventKind::ExtensionReplayMissReason, "unit-test");
        let after = bzlmod_event_counters();

        assert!(count > 0);
        assert!(
            after.extension_replay_miss_reason >= before.extension_replay_miss_reason + 1,
            "extension_replay_miss_reason before={} after={}",
            before.extension_replay_miss_reason,
            after.extension_replay_miss_reason
        );
        assert_eq!(
            BzlmodEventKind::ExtensionReplayMissReason.as_str(),
            "extension_replay_miss_reason"
        );
    }

    #[test]
    fn all_plan61_event_counters_are_observable_in_process() {
        type CounterReader = fn(&BzlmodEventCounters) -> u64;

        let cases: &[(BzlmodEventKind, CounterReader, &str)] = &[
            (
                BzlmodEventKind::BzlmodResolutionCompute,
                |c: &BzlmodEventCounters| c.bzlmod_resolution_compute,
                "bzlmod_resolution_compute",
            ),
            (
                BzlmodEventKind::ModuleFileParse,
                |c: &BzlmodEventCounters| c.module_file_parse,
                "module_file_parse",
            ),
            (
                BzlmodEventKind::ExtensionEval,
                |c: &BzlmodEventCounters| c.extension_eval,
                "extension_eval",
            ),
            (
                BzlmodEventKind::ExtensionReplayHit,
                |c: &BzlmodEventCounters| c.extension_replay_hit,
                "extension_replay_hit",
            ),
            (
                BzlmodEventKind::ExtensionReplayMissReason,
                |c: &BzlmodEventCounters| c.extension_replay_miss_reason,
                "extension_replay_miss_reason",
            ),
            (
                BzlmodEventKind::ExtensionSpokesCompute,
                |c: &BzlmodEventCounters| c.extension_spokes_compute,
                "extension_spokes_compute",
            ),
            (
                BzlmodEventKind::RepoMaterializationStateRead,
                |c: &BzlmodEventCounters| c.repo_materialization_state_read,
                "repo_materialization_state_read",
            ),
            (
                BzlmodEventKind::RepoMaterializationHit,
                |c: &BzlmodEventCounters| c.repo_materialization_hit,
                "repo_materialization_hit",
            ),
            (
                BzlmodEventKind::RepoMaterializationMissReason,
                |c: &BzlmodEventCounters| c.repo_materialization_miss_reason,
                "repo_materialization_miss_reason",
            ),
            (
                BzlmodEventKind::LockfileRead,
                |c: &BzlmodEventCounters| c.lockfile_read,
                "lockfile_read",
            ),
            (
                BzlmodEventKind::LockfileWriteAttempt,
                |c: &BzlmodEventCounters| c.lockfile_write_attempt,
                "lockfile_write_attempt",
            ),
        ];

        for &(kind, read_counter, name) in cases {
            let before = read_counter(&bzlmod_event_counters());
            assert_eq!(kind.as_str(), name);
            record_bzlmod_event(kind, "unit-test");
            let after = read_counter(&bzlmod_event_counters());
            assert!(after >= before + 1, "{name}: before={before} after={after}");
        }
    }

    #[test]
    fn root_module_file_inputs_digest_tracks_digest() {
        let path = Arc::new(PathBuf::from("/tmp/MODULE.bazel"));
        let first = module_file_inputs_digest(&[ModuleFileInputDigest {
            path: path.as_ref().clone(),
            digest: "first".to_owned(),
        }]);
        let second = module_file_inputs_digest(&[ModuleFileInputDigest {
            path: path.as_ref().clone(),
            digest: "second".to_owned(),
        }]);

        assert_ne!(first, second);
    }

    #[test]
    fn root_module_file_inputs_digest_tracks_include_digest() {
        let path = Arc::new(PathBuf::from("/tmp/MODULE.bazel"));
        let root_only = module_file_inputs_digest(&[ModuleFileInputDigest {
            path: path.as_ref().clone(),
            digest: "root".to_owned(),
        }]);
        let with_include = module_file_inputs_digest(&[
            ModuleFileInputDigest {
                path: path.as_ref().clone(),
                digest: "root".to_owned(),
            },
            ModuleFileInputDigest {
                path: PathBuf::from("/tmp/deps.MODULE.bazel"),
                digest: "include".to_owned(),
            },
        ]);
        assert_ne!(root_only, with_include);
    }
}
