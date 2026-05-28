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
use std::hash::Hash;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
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

use crate::extensions::AggregatedExtension;
use crate::lockfile::Lockfile;
use crate::lockfile::LockfileMode;
use crate::parser::ModuleFileInputDigest;
use crate::repo_spec::RepoSpec;
use crate::resolution::ModuleKey;
use crate::resolution::ModuleSource;
use crate::resolution::ResolvedGraph;
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

/// DICE-owned root `MODULE.bazel` read/parse result.
#[derive(Clone, Debug, Allocative)]
pub struct RootModuleFileValue {
    pub path: Arc<PathBuf>,
    pub input_digest: Option<String>,
    pub input_count: usize,
    pub parsed: Option<ParsedModuleFile>,
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
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
}

impl Default for BzlmodLockfileInputsValue {
    fn default() -> Self {
        Self::from_values(None, None, None, crate::LockfileMode::Update)
    }
}

fn lockfile_content_digest(value: &Option<Arc<LockfileContentValue>>) -> Option<String> {
    value.as_ref().and_then(|value| value.digest.clone())
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
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root))
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

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodCellGraphDataValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    #[allocative(skip)]
    pub fallback_cell_graph: Option<Arc<BzlmodCellGraphValue>>,
}

impl BzlmodCellGraphDataValue {
    pub fn for_workspace(
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

    pub fn for_workspace_with_resolved_graph(
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

    pub fn for_workspace_with_resolved_graph_and_fallback(
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

#[derive(derive_more::Display, Debug, Hash, Eq, Clone, PartialEq, Allocative)]
#[display("BzlmodCellGraphDataKey")]
pub(crate) struct BzlmodCellGraphDataKey;

impl dice::InjectedKey for BzlmodCellGraphDataKey {
    type Value = Arc<BzlmodCellGraphDataValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodResolvedGraphDataValue {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
    #[allocative(skip)]
    pub graph: Option<Arc<ResolvedGraph>>,
}

impl BzlmodResolvedGraphDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        resolution_digest: Arc<str>,
        graph: Option<Arc<ResolvedGraph>>,
    ) -> Self {
        Self {
            workspace_id,
            resolution_digest,
            graph,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Allocative)]
pub struct BzlmodResolvedGraphOutputsValue {
    #[allocative(skip)]
    pub graph: Arc<ResolvedGraph>,
    pub graph_digest: Arc<str>,
    pub module_versions: BzlmodModuleVersionsDataValue,
    pub resolution_facts: BzlmodResolutionFactsValue,
    pub registered_toolchains: RegisteredToolchainsDataValue,
    pub registered_execution_platforms: RegisteredExecutionPlatformsDataValue,
    pub extension_aggregations: BzlmodExtensionAggregationsDataValue,
    pub repo_mappings: BzlmodRepoMappingsDataValue,
    pub cell_graph: BzlmodCellGraphValue,
}

#[derive(derive_more::Display, Debug, Hash, Eq, Clone, PartialEq, Allocative)]
#[display("BzlmodResolvedGraphDataKey")]
pub(crate) struct BzlmodResolvedGraphDataKey;

impl dice::InjectedKey for BzlmodResolvedGraphDataKey {
    type Value = Arc<BzlmodResolvedGraphDataValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
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
        if should_use_resolved_graph_data(&self.resolution_digest) {
            let resolved_graph_data = ctx.compute(&BzlmodResolvedGraphDataKey).await?;
            validate_resolved_graph_payload(
                "BzlmodCellDefinitionsKey",
                &self.workspace_id,
                &self.resolution_digest,
                &resolved_graph_data,
            )?;
            let Some(resolved_graph) = resolved_graph_data.graph.as_ref() else {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "BzlmodCellDefinitionsKey expected a resolved graph for project root '{}'",
                    self.workspace_id.canonical_project_root.display()
                ));
            };
            let module_versions = ctx
                .compute(&ModuleVersionsKey::for_workspace_id(
                    self.workspace_id.clone(),
                ))
                .await??;
            let repo_mappings = ctx
                .compute(&BzlmodRepoMappingsKey::for_workspace_id(
                    self.workspace_id.clone(),
                ))
                .await??;
            return Ok(Arc::new(module_cells_from_resolved_graph(
                &self.workspace_id,
                &module_versions.invalidation.root_module_name,
                resolved_graph,
                &repo_mappings,
            )));
        }
        let data = ctx.compute(&BzlmodCellGraphDataKey).await?;
        validate_cell_graph_payload(
            "BzlmodCellDefinitionsKey",
            &self.workspace_id,
            &self.resolution_digest,
            &data,
        )?;
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
        let extension_aggregations = ctx.compute(&BzlmodExtensionAggregationsDataKey).await?;
        if extension_aggregations.workspace_id != self.workspace_id {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "BzlmodExtensionCellDefinitionsKey was computed with project root '{}', \
                 but current bzlmod extension aggregation data root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                extension_aggregations
                    .workspace_id
                    .canonical_project_root
                    .display()
            ));
        }
        if !extension_aggregations.extension_aggregations.is_empty() {
            let repo_env = ctx
                .compute(&BzlmodRepoEnvKey::for_workspace_id(
                    self.workspace_id.clone(),
                ))
                .await??;
            let repo_mappings = ctx
                .compute(&BzlmodRepoMappingsKey::for_workspace_id(
                    self.workspace_id.clone(),
                ))
                .await??;
            return match extension_cells_from_spokes(
                ctx,
                &self.workspace_id,
                extension_aggregations.as_ref(),
                repo_env.as_ref(),
                repo_mappings.as_ref(),
            )
            .await
            {
                Ok(cells) => Ok(cells),
                Err(e) if e.to_string().contains("module extension executor") => {
                    let data = ctx.compute(&BzlmodCellGraphDataKey).await?;
                    validate_cell_graph_payload(
                        "BzlmodExtensionCellDefinitionsKey",
                        &self.workspace_id,
                        &self.resolution_digest,
                        &data,
                    )?;
                    Ok(data.fallback_cell_graph.as_ref().map_or_else(
                        || Arc::new(Vec::new()),
                        |graph| graph.extension_cells.dupe(),
                    ))
                }
                Err(e) => Err(e),
            };
        }
        if should_use_resolved_graph_data(&self.resolution_digest) {
            return Ok(Arc::new(Vec::new()));
        }
        let data = ctx.compute(&BzlmodCellGraphDataKey).await?;
        validate_cell_graph_payload(
            "BzlmodExtensionCellDefinitionsKey",
            &self.workspace_id,
            &self.resolution_digest,
            &data,
        )?;
        Ok(data.fallback_cell_graph.as_ref().map_or_else(
            || Arc::new(Vec::new()),
            |graph| graph.extension_cells.dupe(),
        ))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

async fn extension_cells_from_spokes(
    ctx: &mut DiceComputations<'_>,
    workspace_id: &WorkspaceId,
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
            .compute(&ExtensionSpokesByExtensionIdKey::for_workspace_id(
                workspace_id.clone(),
                extension_id,
            ))
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
        if should_use_resolved_graph_data(&self.resolution_digest) {
            let resolved_graph_data = ctx.compute(&BzlmodResolvedGraphDataKey).await?;
            validate_resolved_graph_payload(
                "BzlmodResidualModuleSymlinksKey",
                &self.workspace_id,
                &self.resolution_digest,
                &resolved_graph_data,
            )?;
            let Some(resolved_graph) = resolved_graph_data.graph.as_ref() else {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "BzlmodResidualModuleSymlinksKey expected a resolved graph for project root '{}'",
                    self.workspace_id.canonical_project_root.display()
                ));
            };
            return Ok(Arc::new(residual_module_symlinks_from_resolved_graph(
                &self.workspace_id,
                resolved_graph,
            )));
        }
        let data = ctx.compute(&BzlmodCellGraphDataKey).await?;
        validate_cell_graph_payload(
            "BzlmodResidualModuleSymlinksKey",
            &self.workspace_id,
            &self.resolution_digest,
            &data,
        )?;
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

fn should_use_resolved_graph_data(resolution_digest: &str) -> bool {
    resolution_digest != INJECTED_BZLMOD_PROJECTION_DIGEST
}

fn validate_resolved_graph_payload(
    key_name: &str,
    workspace_id: &WorkspaceId,
    resolution_digest: &str,
    data: &BzlmodResolvedGraphDataValue,
) -> slug_error::Result<()> {
    if data.workspace_id != *workspace_id {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "{} was computed with project root '{}', \
             but current bzlmod resolved graph root is '{}'",
            key_name,
            workspace_id.canonical_project_root.display(),
            data.workspace_id.canonical_project_root.display()
        ));
    }
    if data.resolution_digest.as_ref() != resolution_digest {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "{} was computed with resolution digest '{}', \
             but current bzlmod resolved graph digest is '{}'",
            key_name,
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
            .compute(&ModuleVersionsKey::for_workspace_id(
                self.workspace_id.clone(),
            ))
            .await??;
        let repo_mappings = ctx
            .compute(&BzlmodRepoMappingsKey::for_workspace_id(
                self.workspace_id.clone(),
            ))
            .await??;
        let extension_aggregations = ctx.compute(&BzlmodExtensionAggregationsDataKey).await?;
        if extension_aggregations.workspace_id != self.workspace_id {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "BzlmodCellGraphKey was computed with project root '{}', \
                 but current bzlmod extension aggregation data root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                extension_aggregations
                    .workspace_id
                    .canonical_project_root
                    .display()
            ));
        }
        let root_module_name = module_versions.invalidation.root_module_name.clone();
        Ok(Arc::new(BzlmodCellGraphValue {
            workspace_id: self.workspace_id.clone(),
            root_module_name: root_module_name.clone(),
            cells: cells.dupe(),
            extension_cells,
            root_aliases: Arc::new(root_aliases_from_repo_mappings(&repo_mappings)),
            module_symlinks: Arc::new(module_symlinks_from_cells_and_residuals(
                cells.as_ref(),
                residual_module_symlinks.as_ref(),
            )),
            scoped_aliases: Arc::new(scoped_aliases_from_repo_mappings(
                &repo_mappings,
                &root_module_name,
            )),
            dynamic_aliases: Arc::new(dynamic_aliases_from_repo_mappings(&repo_mappings)),
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

fn residual_module_symlinks_from_resolved_graph(
    workspace_id: &WorkspaceId,
    resolved_graph: &ResolvedGraph,
) -> Vec<BzlmodCellGraphModuleSymlink> {
    let mut symlinks = Vec::new();
    let mut sorted_modules: Vec<_> = resolved_graph.modules.iter().collect();
    sorted_modules.sort_by(|a, b| a.0.cmp(b.0));
    for (module_name, module_info) in sorted_modules {
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
        let canonical_repo = bazel_canonical_module_repo_name(module_name, &module_info.version);
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

fn bazel_canonical_module_repo_name(module_name: &str, version: &str) -> String {
    if module_name.contains('+') {
        module_name.to_owned()
    } else if version.is_empty() {
        format!("{module_name}+")
    } else {
        format!("{module_name}+")
    }
}

fn module_cells_from_resolved_graph(
    workspace_id: &WorkspaceId,
    root_module_name: &str,
    resolved_graph: &ResolvedGraph,
    repo_mappings: &BzlmodRepoMappingsDataValue,
) -> Vec<BzlmodCellGraphCell> {
    let mut cells = Vec::new();
    let mut sorted_modules: Vec<_> = resolved_graph.modules.iter().collect();
    sorted_modules.sort_by(|a, b| a.0.cmp(b.0));
    for (module_name, module_info) in sorted_modules {
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
                        module_name: module_name.clone(),
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
                        module_name: module_name.clone(),
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
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
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
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
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
    pub lockfile_inputs: Arc<BzlmodLockfileInputsValue>,
}

impl BzlmodLockfileInputsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        lockfile_inputs: Arc<BzlmodLockfileInputsValue>,
    ) -> Self {
        Self {
            workspace_id,
            lockfile_inputs,
        }
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
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
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
    pub repo_env: Arc<BTreeMap<String, String>>,
}

impl BzlmodRepoEnvDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        repo_env: Arc<BTreeMap<String, String>>,
    ) -> Self {
        Self {
            workspace_id,
            repo_env,
        }
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
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
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
            registry_file_hashes,
            selected_yanked_versions,
        }
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
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
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
            root_module_name,
            module_versions,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodRepoMappingsDataValue {
    pub workspace_id: WorkspaceId,
    pub repo_mappings: Arc<crate::RepoMappingSnapshot>,
    pub repo_mapping_overrides: Arc<crate::RepoMappingOverrides>,
}

impl BzlmodRepoMappingsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        repo_mappings: Arc<crate::RepoMappingSnapshot>,
        repo_mapping_overrides: Arc<crate::RepoMappingOverrides>,
    ) -> Self {
        Self {
            workspace_id,
            repo_mappings,
            repo_mapping_overrides,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodExtensionAggregationsDataValue {
    pub workspace_id: WorkspaceId,
    pub root_module_name: String,
    pub extension_aggregations: Arc<HashMap<String, AggregatedExtension>>,
}

impl BzlmodExtensionAggregationsDataValue {
    pub fn for_workspace_with_root_module_name(
        workspace_id: WorkspaceId,
        root_module_name: String,
        extension_aggregations: Arc<HashMap<String, AggregatedExtension>>,
    ) -> Self {
        Self {
            workspace_id,
            root_module_name,
            extension_aggregations,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodExtensionAggregationValue {
    pub workspace_id: WorkspaceId,
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
        Ok(data.lockfile_inputs.clone())
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
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
        let lockfile_inputs = ctx
            .compute(&BzlmodLockfileInputsKey::for_workspace_id(
                self.workspace_id.clone(),
            ))
            .await??;
        let repo_env = ctx
            .compute(&BzlmodRepoEnvKey::for_workspace_id(
                self.workspace_id.clone(),
            ))
            .await??;
        let repo_mappings = ctx
            .compute(&BzlmodRepoMappingsKey::for_workspace_id(
                self.workspace_id.clone(),
            ))
            .await??;
        let resolution_facts = ctx
            .compute(&BzlmodResolutionFactsKey::for_workspace_id(
                self.workspace_id.clone(),
            ))
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
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
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
    pub registered_toolchains: Vec<crate::RegisteredToolchain>,
}

impl RegisteredToolchainsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        registered_toolchains: Vec<crate::RegisteredToolchain>,
    ) -> Self {
        Self {
            workspace_id,
            registered_toolchains,
        }
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
    pub fn for_workspace_id(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            resolution_digest: Arc::from(INJECTED_BZLMOD_PROJECTION_DIGEST),
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
    pub registered_execution_platforms: Vec<String>,
}

impl RegisteredExecutionPlatformsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        registered_execution_platforms: Vec<String>,
    ) -> Self {
        Self {
            workspace_id,
            registered_execution_platforms,
        }
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
    "ExtensionBzlTransitiveDigestKey({}, {})",
    workspace_id.stable_hash(),
    extension_id
)]
pub struct ExtensionBzlTransitiveDigestKey {
    pub workspace_id: WorkspaceId,
    pub extension_id: Arc<str>,
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "BzlmodExtensionAggregationKey({}, {})",
    workspace_id.stable_hash(),
    extension_id
)]
pub struct BzlmodExtensionAggregationKey {
    pub workspace_id: WorkspaceId,
    pub extension_id: Arc<str>,
}

impl BzlmodExtensionAggregationKey {
    pub fn for_workspace_id(workspace_id: WorkspaceId, extension_id: &str) -> Self {
        Self {
            workspace_id,
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
    "ExtensionSpokesKey({}, {}, {})",
    workspace_id.stable_hash(),
    extension_id,
    bzl_transitive_digest
)]
pub struct ExtensionSpokesKey {
    pub workspace_id: WorkspaceId,
    pub extension_id: Arc<str>,
    pub bzl_transitive_digest: Arc<str>,
}

impl ExtensionSpokesKey {
    pub fn for_workspace_id(workspace_id: WorkspaceId, extension_id: &str) -> Self {
        Self::for_workspace_id_with_digest(workspace_id, extension_id, "")
    }

    pub fn for_workspace_id_with_digest(
        workspace_id: WorkspaceId,
        extension_id: &str,
        bzl_transitive_digest: &str,
    ) -> Self {
        Self {
            workspace_id,
            extension_id: Arc::from(extension_id),
            bzl_transitive_digest: Arc::from(bzl_transitive_digest),
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf, extension_id: &str) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root), extension_id)
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "ExtensionSpokesByExtensionIdKey({}, {})",
    workspace_id.stable_hash(),
    extension_id
)]
pub struct ExtensionSpokesByExtensionIdKey {
    pub workspace_id: WorkspaceId,
    pub extension_id: Arc<str>,
}

impl ExtensionSpokesByExtensionIdKey {
    pub fn for_workspace_id(workspace_id: WorkspaceId, extension_id: &str) -> Self {
        Self {
            workspace_id,
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
    "ExtensionIdByCanonicalRepoKey({}, {})",
    workspace_id.stable_hash(),
    canonical_name
)]
pub struct ExtensionIdByCanonicalRepoKey {
    pub workspace_id: WorkspaceId,
    pub canonical_name: Arc<str>,
}

impl ExtensionIdByCanonicalRepoKey {
    pub fn for_workspace_id(workspace_id: WorkspaceId, canonical_name: &str) -> Self {
        Self {
            workspace_id,
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
    "ExtensionSpokesByCanonicalRepoKey({}, {})",
    workspace_id.stable_hash(),
    canonical_name
)]
pub struct ExtensionSpokesByCanonicalRepoKey {
    pub workspace_id: WorkspaceId,
    pub canonical_name: Arc<str>,
}

impl ExtensionSpokesByCanonicalRepoKey {
    pub fn for_workspace_id(workspace_id: WorkspaceId, canonical_name: &str) -> Self {
        Self {
            workspace_id,
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

    pub fn for_workspace_id_with_repo_spec_digest_and_repo_env(
        workspace_id: WorkspaceId,
        canonical_repo: &str,
        repo_spec: Arc<RepoSpec>,
        repo_spec_digest: String,
        repo_env: Arc<BTreeMap<String, String>>,
    ) -> Self {
        Self {
            output_base: workspace_id.output_base.clone(),
            workspace_id,
            canonical_repo: Arc::from(canonical_repo),
            repo_spec_digest: Arc::from(repo_spec_digest.as_str()),
            repo_spec,
            repo_env,
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
    }
}

impl std::hash::Hash for RepoMaterializationManifestKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.workspace_id.hash(state);
        self.output_base.hash(state);
        self.canonical_repo.hash(state);
        self.repo_spec_digest.hash(state);
        self.repo_env.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct RepoMaterializationManifestValue {
    pub key: RepoMaterializationManifestKey,
    pub repo_dir: Arc<PathBuf>,
    pub marker_state: Arc<str>,
    pub layout_state: Arc<str>,
    pub recorded_inputs_state: Arc<str>,
    pub digest: Arc<str>,
}

impl RepoMaterializationManifestValue {
    pub fn new(
        key: RepoMaterializationManifestKey,
        repo_dir: PathBuf,
        marker_state: String,
        layout_state: String,
        recorded_inputs_state: String,
    ) -> Self {
        let digest = repo_materialization_manifest_digest(
            &key,
            &marker_state,
            &layout_state,
            &recorded_inputs_state,
        );
        Self {
            key,
            repo_dir: Arc::new(repo_dir),
            marker_state: Arc::from(marker_state.as_str()),
            layout_state: Arc::from(layout_state.as_str()),
            recorded_inputs_state: Arc::from(recorded_inputs_state.as_str()),
            digest: Arc::from(digest.as_str()),
        }
    }

    pub fn state_summary(&self) -> String {
        format!(
            "{};{};{}",
            self.marker_state, self.layout_state, self.recorded_inputs_state
        )
    }
}

fn repo_materialization_manifest_digest(
    key: &RepoMaterializationManifestKey,
    marker_state: &str,
    layout_state: &str,
    recorded_inputs_state: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"repo-materialization-manifest-v1");
    hasher.update([0]);
    update_digest_str(&mut hasher, key.workspace_id.stable_hash());
    update_digest_str(&mut hasher, &key.canonical_repo);
    update_digest_str(&mut hasher, &key.repo_spec_digest);
    update_digest_str(&mut hasher, marker_state);
    update_digest_str(&mut hasher, layout_state);
    update_digest_str(&mut hasher, recorded_inputs_state);
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
        repo_materialization_hit: REPO_MATERIALIZATION_HIT.load(Ordering::Relaxed),
        repo_materialization_miss_reason: REPO_MATERIALIZATION_MISS_REASON.load(Ordering::Relaxed),
        lockfile_read: LOCKFILE_READ.load(Ordering::Relaxed),
        lockfile_write_attempt: LOCKFILE_WRITE_ATTEMPT.load(Ordering::Relaxed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
