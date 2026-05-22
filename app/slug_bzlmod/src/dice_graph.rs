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
use sha2::Digest;
use sha2::Sha256;

use crate::lockfile::Lockfile;
use crate::lockfile::compute_file_hash;
use crate::lockfile::compute_sha256_hex;
use crate::parser::ModuleFileInputDigest;
use crate::parser::parse_module_bazel_content_from_path;
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

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "RootModuleFileKey({})",
    workspace_id.canonical_project_root.display()
)]
pub struct RootModuleFileKey {
    pub workspace_id: WorkspaceId,
}

/// DICE-owned root `MODULE.bazel` read/parse result.
///
/// This is a narrow Plan 61 bridge: root module parsing moves into DICE before
/// the broader MVS/cell-graph migration. Like `LockfileContentKey`, this key is
/// deliberately non-cacheable until the file read is backed by tracked DICE
/// filesystem inputs.
#[derive(Clone, Debug, Allocative)]
pub struct RootModuleFileValue {
    pub path: Arc<PathBuf>,
    pub input_digest: Option<String>,
    pub input_count: usize,
    pub parsed: Option<ParsedModuleFile>,
}

#[async_trait]
impl Key for RootModuleFileKey {
    type Value = slug_error::Result<Arc<RootModuleFileValue>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let path = Arc::new(
            self.workspace_id
                .canonical_project_root
                .join("MODULE.bazel"),
        );
        if !path.exists() {
            return Ok(Arc::new(RootModuleFileValue {
                path,
                input_digest: None,
                input_count: 0,
                parsed: None,
            }));
        }

        let content = std::fs::read(path.as_ref())?;
        let digest = compute_sha256_hex(&content);
        let content = String::from_utf8(content).map_err(|e| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Failed to read MODULE.bazel at {:?}: {}",
                path,
                e
            )
        })?;
        let parsed_with_inputs =
            parse_module_bazel_content_from_path(path.as_ref(), &content, digest)?;
        let input_digest = module_file_inputs_digest(&parsed_with_inputs.inputs);
        let input_count = parsed_with_inputs.inputs.len();

        Ok(Arc::new(RootModuleFileValue {
            path,
            input_digest: Some(input_digest),
            input_count,
            parsed: Some(parsed_with_inputs.parsed),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.path == y.path && x.input_digest == y.input_digest,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        false
    }
}

fn module_file_inputs_digest(inputs: &[ModuleFileInputDigest]) -> String {
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

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("LockfileContentKey({:?}, {})", kind, path.display())]
pub struct LockfileContentKey {
    pub workspace_id: WorkspaceId,
    pub kind: LockfileContentKind,
    pub path: Arc<PathBuf>,
}

/// DICE-owned lockfile read result.
///
/// This key is deliberately non-cacheable until Slug wires lockfile reads
/// through filesystem-tracked DICE inputs. That keeps command behavior from
/// reusing stale lockfile bytes while moving consumers away from ad hoc direct
/// reads inside higher-level bzlmod keys.
#[derive(Clone, Debug, Allocative)]
pub struct LockfileContentValue {
    pub path: Arc<PathBuf>,
    pub digest: Option<String>,
    #[allocative(skip)]
    pub lockfile: Option<Arc<Lockfile>>,
}

#[async_trait]
impl Key for LockfileContentKey {
    type Value = slug_error::Result<Arc<LockfileContentValue>>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let path = self.path.clone();
        if !path.exists() {
            return Ok(Arc::new(LockfileContentValue {
                path,
                digest: None,
                lockfile: None,
            }));
        }

        let read_result = Lockfile::read(&path).and_then(|lockfile| {
            let digest = compute_file_hash(&path)?;
            Ok((lockfile, digest))
        });

        match read_result {
            Ok((lockfile, digest)) => Ok(Arc::new(LockfileContentValue {
                path,
                digest: Some(digest),
                lockfile: Some(Arc::new(lockfile)),
            })),
            Err(e) if self.kind == LockfileContentKind::Hidden => {
                tracing::warn!(
                    "Ignoring unreadable hidden lockfile '{}': {}",
                    path.display(),
                    e
                );
                Ok(Arc::new(LockfileContentValue {
                    path,
                    digest: None,
                    lockfile: None,
                }))
            }
            Err(e) => Err(e),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.digest == y.digest && x.path == y.path,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        // Recompute every request until this key depends on tracked file inputs.
        false
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct BzlmodCellGraphKey {
    pub workspace_id: WorkspaceId,
    pub resolution_digest: Arc<str>,
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
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self {
            workspace_id: WorkspaceId::for_project_root(project_root),
            resolution_digest: Arc::from("injected-bzlmod-session"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct ModuleVersionsValue {
    pub workspace_id: WorkspaceId,
    pub module_versions: Arc<HashMap<String, String>>,
}

#[async_trait]
impl Key for ModuleVersionsKey {
    type Value = slug_error::Result<Arc<ModuleVersionsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let session_data = ctx.compute(&crate::BzlmodSessionDataKey).await?;
        if session_data.project_root != *self.workspace_id.canonical_project_root {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "ModuleVersionsKey was computed with project root '{}', \
                 but current bzlmod session root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                session_data.project_root.display()
            ));
        }
        Ok(Arc::new(ModuleVersionsValue {
            workspace_id: self.workspace_id.clone(),
            module_versions: Arc::new(session_data.module_versions.clone()),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        let _ = (x, y);
        // Transitional bridge: the interpreter previously depended directly
        // on the whole injected `BzlmodSessionData`. Do not narrow cutoffs to
        // the version map until the remaining bzlmod session fields have
        // explicit interpreter/materialization dependencies.
        false
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
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self {
            workspace_id: WorkspaceId::for_project_root(project_root),
            resolution_digest: Arc::from("injected-bzlmod-session"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct RegisteredToolchainsValue {
    pub workspace_id: WorkspaceId,
    pub registered_toolchains: Vec<crate::RegisteredToolchain>,
}

#[async_trait]
impl Key for RegisteredToolchainsKey {
    type Value = slug_error::Result<Arc<RegisteredToolchainsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let session_data = ctx.compute(&crate::BzlmodSessionDataKey).await?;
        if session_data.project_root != *self.workspace_id.canonical_project_root {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "RegisteredToolchainsKey was computed with project root '{}', \
                 but current bzlmod session root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                session_data.project_root.display()
            ));
        }
        Ok(Arc::new(RegisteredToolchainsValue {
            workspace_id: self.workspace_id.clone(),
            registered_toolchains: session_data.registered_toolchains.clone(),
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
    pub fn for_project_root(project_root: PathBuf) -> Self {
        Self {
            workspace_id: WorkspaceId::for_project_root(project_root),
            resolution_digest: Arc::from("injected-bzlmod-session"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct RegisteredExecutionPlatformsValue {
    pub workspace_id: WorkspaceId,
    pub registered_execution_platforms: Vec<String>,
}

#[async_trait]
impl Key for RegisteredExecutionPlatformsKey {
    type Value = slug_error::Result<Arc<RegisteredExecutionPlatformsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let session_data = ctx.compute(&crate::BzlmodSessionDataKey).await?;
        if session_data.project_root != *self.workspace_id.canonical_project_root {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "RegisteredExecutionPlatformsKey was computed with project root '{}', \
                 but current bzlmod session root is '{}'",
                self.workspace_id.canonical_project_root.display(),
                session_data.project_root.display()
            ));
        }
        Ok(Arc::new(RegisteredExecutionPlatformsValue {
            workspace_id: self.workspace_id.clone(),
            registered_execution_platforms: session_data.registered_execution_platforms.clone(),
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
#[display("ExtensionSpokesKey({}, {})", workspace_id.stable_hash(), extension_id)]
pub struct ExtensionSpokesKey {
    pub workspace_id: WorkspaceId,
    pub extension_id: Arc<str>,
}

impl ExtensionSpokesKey {
    pub fn for_workspace_id(workspace_id: WorkspaceId, extension_id: &str) -> Self {
        Self {
            workspace_id,
            extension_id: Arc::from(extension_id),
        }
    }

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
    pub fn for_project_root(project_root: PathBuf, extension_id: &str) -> Self {
        Self {
            workspace_id: WorkspaceId::for_project_root(project_root),
            extension_id: Arc::from(extension_id),
        }
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
pub struct RepoMaterializationManifestKey {
    pub workspace_id: WorkspaceId,
    pub output_base: Arc<PathBuf>,
    pub canonical_repo: Arc<str>,
    pub repo_spec_digest: Arc<str>,
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
    fn lockfile_content_key_is_non_cacheable_until_file_deps_are_tracked() {
        let value = Ok(Arc::new(LockfileContentValue {
            path: Arc::new(PathBuf::from("/tmp/MODULE.bazel.lock")),
            digest: None,
            lockfile: None,
        }));

        assert!(!<LockfileContentKey as Key>::validity(&value));
    }

    #[test]
    fn root_module_file_key_is_non_cacheable_until_file_deps_are_tracked() {
        let value = Ok(Arc::new(RootModuleFileValue {
            path: Arc::new(PathBuf::from("/tmp/MODULE.bazel")),
            input_digest: None,
            input_count: 0,
            parsed: None,
        }));

        assert!(!<RootModuleFileKey as Key>::validity(&value));
    }

    #[test]
    fn root_module_file_value_equality_tracks_digest() {
        let path = Arc::new(PathBuf::from("/tmp/MODULE.bazel"));
        let first = Ok(Arc::new(RootModuleFileValue {
            path: path.clone(),
            input_digest: Some("first".to_owned()),
            input_count: 1,
            parsed: None,
        }));
        let second = Ok(Arc::new(RootModuleFileValue {
            path,
            input_digest: Some("second".to_owned()),
            input_count: 1,
            parsed: None,
        }));

        assert!(!<RootModuleFileKey as Key>::equality(&first, &second));
    }

    #[test]
    fn root_module_file_value_equality_tracks_include_digest() {
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
        let first = Ok(Arc::new(RootModuleFileValue {
            path: path.clone(),
            input_digest: Some(root_only),
            input_count: 1,
            parsed: None,
        }));
        let second = Ok(Arc::new(RootModuleFileValue {
            path,
            input_digest: Some(with_include),
            input_count: 2,
            parsed: None,
        }));

        assert!(!<RootModuleFileKey as Key>::equality(&first, &second));
    }
}
