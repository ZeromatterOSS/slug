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
use crate::parser::ModuleFileInputDigest;
use crate::repo_spec::RepoSpec;
use crate::resolution::ModuleKey;
use crate::resolution::ModuleSource;
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
            resolution_digest: Arc::from("injected-bzlmod-session"),
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
#[display("BzlmodCellGraphDataKey")]
pub struct BzlmodCellGraphDataKey;

impl dice::InjectedKey for BzlmodCellGraphDataKey {
    type Value = Arc<BzlmodCellGraphValue>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for BzlmodCellGraphKey {
    type Value = slug_error::Result<Arc<BzlmodCellGraphValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let value = ctx.compute(&BzlmodCellGraphDataKey).await?;
        if value.workspace_id != self.workspace_id {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "BzlmodCellGraphKey was computed with project root '{}', \
                 but current bzlmod cell graph root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                value.workspace_id.canonical_project_root.display()
            ));
        }
        Ok(value)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
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
            resolution_digest: Arc::from("injected-bzlmod-session"),
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
            resolution_digest: Arc::from("injected-bzlmod-session"),
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct BzlmodLockfileInputsDataValue {
    pub workspace_id: Option<WorkspaceId>,
    pub lockfile_inputs: Arc<BzlmodLockfileInputsValue>,
}

impl BzlmodLockfileInputsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        lockfile_inputs: Arc<BzlmodLockfileInputsValue>,
    ) -> Self {
        Self {
            workspace_id: Some(workspace_id),
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
            resolution_digest: Arc::from("injected-bzlmod-session"),
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Allocative)]
pub struct BzlmodRepoEnvDataValue {
    pub workspace_id: Option<WorkspaceId>,
    pub repo_env: Arc<BTreeMap<String, String>>,
}

impl BzlmodRepoEnvDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        repo_env: Arc<BTreeMap<String, String>>,
    ) -> Self {
        Self {
            workspace_id: Some(workspace_id),
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
            resolution_digest: Arc::from("injected-bzlmod-session"),
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Allocative)]
pub struct BzlmodResolutionFactsValue {
    pub workspace_id: Option<WorkspaceId>,
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
            workspace_id: Some(workspace_id),
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
            resolution_digest: Arc::from("injected-bzlmod-session"),
        }
    }

    #[cfg(test)]
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace_id(WorkspaceId::for_project_root(project_root))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Allocative)]
pub struct BzlmodModuleVersionsDataValue {
    pub workspace_id: Option<WorkspaceId>,
    pub module_versions: Arc<HashMap<String, String>>,
}

impl BzlmodModuleVersionsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        module_versions: Arc<HashMap<String, String>>,
    ) -> Self {
        Self {
            workspace_id: Some(workspace_id),
            module_versions,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Allocative)]
pub struct BzlmodRepoMappingsDataValue {
    pub workspace_id: Option<WorkspaceId>,
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
            workspace_id: Some(workspace_id),
            repo_mappings,
            repo_mapping_overrides,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Allocative)]
pub struct BzlmodExtensionAggregationsDataValue {
    pub workspace_id: Option<WorkspaceId>,
    pub extension_aggregations: Arc<HashMap<String, AggregatedExtension>>,
}

impl BzlmodExtensionAggregationsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        extension_aggregations: Arc<HashMap<String, AggregatedExtension>>,
    ) -> Self {
        Self {
            workspace_id: Some(workspace_id),
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
        // monolithic session dependency, but it still carries a conservative
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
        if let Some(data_workspace_id) = data.workspace_id.as_ref() {
            if data_workspace_id != &self.workspace_id {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "BzlmodLockfileInputsKey was computed with project root '{}', \
                     but current bzlmod lockfile input data root is '{}'",
                    self.workspace_id.canonical_project_root.display(),
                    data_workspace_id.canonical_project_root.display()
                ));
            }
        }
        ctx.compute(&BzlmodCellGraphKey::for_workspace_id(
            self.workspace_id.clone(),
        ))
        .await??;
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
        if let Some(data_workspace_id) = data.workspace_id.as_ref() {
            if data_workspace_id != &self.workspace_id {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "BzlmodRepoEnvKey was computed with project root '{}', \
                     but current bzlmod repo env data root is '{}'",
                    self.workspace_id.canonical_project_root.display(),
                    data_workspace_id.canonical_project_root.display()
                ));
            }
        }
        ctx.compute(&BzlmodCellGraphKey::for_workspace_id(
            self.workspace_id.clone(),
        ))
        .await??;
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
        if let Some(data_workspace_id) = data.workspace_id.as_ref() {
            if data_workspace_id != &self.workspace_id {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "BzlmodRepoMappingsKey was computed with project root '{}', \
                     but current bzlmod repo mapping data root is '{}'",
                    self.workspace_id.canonical_project_root.display(),
                    data_workspace_id.canonical_project_root.display()
                ));
            }
        }
        ctx.compute(&BzlmodCellGraphKey::for_workspace_id(
            self.workspace_id.clone(),
        ))
        .await??;
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
        if let Some(data_workspace_id) = data.workspace_id.as_ref() {
            if data_workspace_id != &self.workspace_id {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "BzlmodResolutionFactsKey was computed with project root '{}', \
                     but current bzlmod resolution facts data root is '{}'",
                    self.workspace_id.canonical_project_root.display(),
                    data_workspace_id.canonical_project_root.display()
                ));
            }
        }
        ctx.compute(&BzlmodCellGraphKey::for_workspace_id(
            self.workspace_id.clone(),
        ))
        .await??;
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
        if let Some(data_workspace_id) = data.workspace_id.as_ref() {
            if data_workspace_id != &self.workspace_id {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "ModuleVersionsKey was computed with project root '{}', \
                     but current bzlmod module versions data root is '{}'",
                    self.workspace_id.canonical_project_root.display(),
                    data_workspace_id.canonical_project_root.display()
                ));
            }
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
        let cell_graph = ctx
            .compute(&BzlmodCellGraphKey::for_workspace_id(
                self.workspace_id.clone(),
            ))
            .await??;
        let resolution_facts = ctx
            .compute(&BzlmodResolutionFactsKey::for_workspace_id(
                self.workspace_id.clone(),
            ))
            .await??;
        Ok(Arc::new(ModuleVersionsValue {
            workspace_id: cell_graph.workspace_id.clone(),
            module_versions: data.module_versions.clone(),
            invalidation: Arc::new(BzlmodModuleVersionsInvalidation {
                root_module_name: cell_graph.root_module_name.clone(),
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
            resolution_digest: Arc::from("injected-bzlmod-session"),
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Allocative)]
pub struct RegisteredToolchainsDataValue {
    pub workspace_id: Option<WorkspaceId>,
    pub registered_toolchains: Vec<crate::RegisteredToolchain>,
}

impl RegisteredToolchainsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        registered_toolchains: Vec<crate::RegisteredToolchain>,
    ) -> Self {
        Self {
            workspace_id: Some(workspace_id),
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
        if let Some(data_workspace_id) = data.workspace_id.as_ref() {
            if data_workspace_id != &self.workspace_id {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "RegisteredToolchainsKey was computed with project root '{}', \
                     but current bzlmod registered toolchain data root is '{}'",
                    self.workspace_id.canonical_project_root.display(),
                    data_workspace_id.canonical_project_root.display()
                ));
            }
        }
        let cell_graph = ctx
            .compute(&BzlmodCellGraphKey::for_workspace_id(
                self.workspace_id.clone(),
            ))
            .await??;
        Ok(Arc::new(RegisteredToolchainsValue {
            workspace_id: cell_graph.workspace_id.clone(),
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
            resolution_digest: Arc::from("injected-bzlmod-session"),
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Allocative)]
pub struct RegisteredExecutionPlatformsDataValue {
    pub workspace_id: Option<WorkspaceId>,
    pub registered_execution_platforms: Vec<String>,
}

impl RegisteredExecutionPlatformsDataValue {
    pub fn for_workspace(
        workspace_id: WorkspaceId,
        registered_execution_platforms: Vec<String>,
    ) -> Self {
        Self {
            workspace_id: Some(workspace_id),
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
        if let Some(data_workspace_id) = data.workspace_id.as_ref() {
            if data_workspace_id != &self.workspace_id {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "RegisteredExecutionPlatformsKey was computed with project root '{}', \
                     but current bzlmod registered execution platform data root is '{}'",
                    self.workspace_id.canonical_project_root.display(),
                    data_workspace_id.canonical_project_root.display()
                ));
            }
        }
        let cell_graph = ctx
            .compute(&BzlmodCellGraphKey::for_workspace_id(
                self.workspace_id.clone(),
            ))
            .await??;
        Ok(Arc::new(RegisteredExecutionPlatformsValue {
            workspace_id: cell_graph.workspace_id.clone(),
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

    pub fn for_workspace_id_with_repo_spec_digest(
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
