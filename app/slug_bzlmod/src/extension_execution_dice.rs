/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! DICE-based module extension execution.
//!
//! This module provides DICE keys for evaluating module extensions. Extensions
//! are evaluated to capture `RepoSpec` objects (deferred execution model) - no
//! actual downloads happen during extension evaluation.
//!
//! ## Deferred Execution Model
//!
//! When a module extension is evaluated:
//! 1. A temporary working directory is created for `module_ctx` I/O
//! 2. The extension implementation is called with `module_ctx`
//! 3. Repository rule calls capture `RepoSpec` objects (NOT executed)
//! 4. The temporary directory is cleaned up
//! 5. `ModuleExtensionResult` is returned with all captured specs
//!
//! Actual repository materialization happens later via `ExtensionRepoExecutionKey`
//! when repositories are first accessed during a build.
//!
//! ## Pattern Reference
//!
//! This follows the `RepositoryRuleExecutionKey` pattern from `repository_execution.rs`.

use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dupe::Dupe;
use fxhash::FxHashMap;
use serde::Serialize;
#[cfg(test)]
use starlark::syntax::AstModule;
#[cfg(test)]
use starlark::syntax::Dialect;
#[cfg(test)]
use starlark_syntax::syntax::ast::AstStmt;
#[cfg(test)]
use starlark_syntax::syntax::ast::StmtP;

use crate::BzlmodExtensionAggregationValue;
use crate::BzlmodExtensionAggregationsDataValue;
use crate::RepoMappingOverrides;
use crate::RepoMappingSnapshot;
use crate::dice_graph::BzlmodEventKind;
use crate::dice_graph::BzlmodExtensionAggregationKey;
use crate::dice_graph::BzlmodExtensionAggregationsDataKey;
use crate::dice_graph::BzlmodLockfileInputsKey;
use crate::dice_graph::BzlmodLockfileInputsValue;
use crate::dice_graph::BzlmodRepoEnvKey;
use crate::dice_graph::ExtensionBzlTransitiveDigestKey;
use crate::dice_graph::ExtensionIdByCanonicalRepoKey;
use crate::dice_graph::ExtensionSpoke;
use crate::dice_graph::ExtensionSpokesByCanonicalRepoKey;
use crate::dice_graph::ExtensionSpokesByExtensionIdKey;
use crate::dice_graph::ExtensionSpokesKey;
use crate::dice_graph::ExtensionSpokesValue;
use crate::dice_graph::LockfileContentValue;
use crate::dice_graph::record_bzlmod_event;
use crate::extensions::AggregatedExtension;
use crate::extensions::compute_extension_input_hash;
use crate::lockfile::LockfileMode;
use crate::lockfile::SelectedExtensionCache;
use crate::lockfile::compute_sha256_hex;
use crate::module_extension_executor::MODULE_EXTENSION_EXECUTOR_IMPL;
use crate::module_extension_executor::ModuleExtensionMetadata;
use crate::repo_spec::RepoSpec;
use crate::repository_execution::REPOSITORY_MATERIALIZATION_STATE_READER_IMPL;
use crate::repository_execution::validate_recorded_inputs_with_dice_reader;

fn stable_json_digest<T: Serialize>(value: &T) -> String {
    let json = serde_json::to_string(value).unwrap_or_else(|_| "<json-error>".to_owned());
    compute_sha256_hex(json.as_bytes())
}

fn stable_json_string<T: Serialize>(value: &T, fallback: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| fallback.to_owned())
}

pub fn repo_mappings_identity_digest(repo_mappings: &RepoMappingSnapshot) -> String {
    stable_json_digest(repo_mappings)
}

pub fn repo_mapping_overrides_identity_digest(overrides: &RepoMappingOverrides) -> String {
    stable_json_digest(overrides)
}

fn create_extension_execution_key_from_aggregation(
    aggregation: &BzlmodExtensionAggregationValue,
    repo_env: &BTreeMap<String, String>,
    replay_inputs: Arc<ModuleExtensionReplayInputsValue>,
    repo_mappings: &RepoMappingSnapshot,
    repo_mapping_overrides: &RepoMappingOverrides,
    bzl_transitive_digest: Arc<str>,
) -> ModuleExtensionExecutionKey {
    ModuleExtensionExecutionKey::new_with_replay_inputs_and_bzl_digest(
        aggregation.aggregated.as_ref().clone(),
        aggregation.root_module_name.to_string(),
        aggregation
            .workspace_id
            .canonical_project_root
            .as_ref()
            .clone(),
        replay_inputs,
        repo_env.clone(),
        repo_mappings.clone(),
        repo_mapping_overrides.clone(),
        bzl_transitive_digest,
        aggregation.workspace_id.clone(),
    )
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Allocative)]
struct SelectedExtensionCacheIdentity {
    source: crate::dice_graph::LockfileContentKind,
    selected_key: String,
    repo_specs_digest: String,
    recorded_inputs: Vec<String>,
    workspace_root: Option<PathBuf>,
    repo_env: Option<BTreeMap<String, String>>,
    repo_mappings: Option<RepoMappingSnapshot>,
}

#[derive(Clone, Debug, Allocative)]
pub struct ModuleExtensionReplayInputsValue {
    pub lockfile_mode: LockfileMode,
    #[allocative(skip)]
    pub prior_facts: serde_json::Value,
    #[allocative(skip)]
    pub workspace_lockfile_facts: serde_json::Value,
    pub workspace_lockfile_facts_present: bool,
    #[allocative(skip)]
    pub selected_cache: Option<SelectedExtensionCache>,
    selected_cache_identity: Option<SelectedExtensionCacheIdentity>,
    identity_digest: Arc<str>,
}

impl ModuleExtensionReplayInputsValue {
    #[cfg(test)]
    fn empty(lockfile_mode: LockfileMode) -> Arc<Self> {
        Self::from_parts(
            lockfile_mode,
            empty_facts(),
            empty_facts(),
            false,
            None,
            None,
        )
    }

    fn from_lockfile_inputs(
        extension_id: &str,
        bzl_transitive_digest: &str,
        usages_digest: &str,
        project_root: Option<&Path>,
        root_module_name: &str,
        repo_env: &BTreeMap<String, String>,
        repo_mappings: &RepoMappingSnapshot,
        repo_mapping_overrides: &RepoMappingOverrides,
        lockfile_inputs: &BzlmodLockfileInputsValue,
    ) -> slug_error::Result<Arc<Self>> {
        let mut prior_facts = empty_facts();
        let mut workspace_lockfile_facts = empty_facts();
        let mut workspace_lockfile_facts_present = false;
        let mut selected_cache = None;
        let mut selected_cache_identity = None;

        if lockfile_inputs.lockfile_mode != LockfileMode::Off {
            if let Some(project_root) = project_root
                && let Some(lockfile_value) = &lockfile_inputs.visible_lockfile
            {
                if let Some(lockfile) = lockfile_value.lockfile.as_ref() {
                    verify_observed_lockfile_digest(
                        lockfile_value,
                        lockfile_inputs.visible_lockfile_digest.as_deref(),
                        "workspace lockfile",
                    )?;
                    if let Some(facts) = lockfile.get_extension_facts(extension_id) {
                        prior_facts = facts.clone();
                        workspace_lockfile_facts = facts;
                        workspace_lockfile_facts_present = true;
                    }
                    match lockfile.select_extension_cache_for_workspace(
                        extension_id,
                        bzl_transitive_digest,
                        usages_digest,
                        Some(project_root),
                        Some(repo_env),
                        Some(repo_mappings),
                        Some(root_module_name),
                        Some(repo_mapping_overrides),
                    ) {
                        Some(cache) => {
                            selected_cache_identity = Some(selected_extension_cache_identity(
                                crate::dice_graph::LockfileContentKind::Workspace,
                                &cache,
                            ));
                            selected_cache = Some(cache);
                        }
                        None => {
                            record_bzlmod_event(
                                BzlmodEventKind::ExtensionReplayMissReason,
                                format!("{extension_id}:digest_or_entry_miss"),
                            );
                            tracing::debug!(
                                "Extension '{}' cache MISS: digests don't match",
                                extension_id
                            );
                        }
                    }
                } else {
                    record_bzlmod_event(
                        BzlmodEventKind::ExtensionReplayMissReason,
                        format!("{extension_id}:lockfile_absent_or_unreadable"),
                    );
                }
            }

            if selected_cache.is_none()
                && let Some(lockfile_value) = &lockfile_inputs.hidden_lockfile
                && let Some(lockfile) = lockfile_value.lockfile.as_ref()
            {
                verify_observed_lockfile_digest(
                    lockfile_value,
                    lockfile_inputs.hidden_lockfile_digest.as_deref(),
                    "hidden lockfile",
                )?;
                if !workspace_lockfile_facts_present {
                    prior_facts = lockfile
                        .get_extension_facts(extension_id)
                        .unwrap_or_else(empty_facts);
                }
                if let Some(cache) = lockfile.select_extension_cache_for_workspace(
                    extension_id,
                    bzl_transitive_digest,
                    usages_digest,
                    project_root,
                    Some(repo_env),
                    Some(repo_mappings),
                    Some(root_module_name),
                    Some(repo_mapping_overrides),
                ) {
                    selected_cache_identity = Some(selected_extension_cache_identity(
                        crate::dice_graph::LockfileContentKind::Hidden,
                        &cache,
                    ));
                    selected_cache = Some(cache);
                }
            }
        }

        Ok(Self::from_parts(
            lockfile_inputs.lockfile_mode,
            prior_facts,
            workspace_lockfile_facts,
            workspace_lockfile_facts_present,
            selected_cache,
            selected_cache_identity,
        ))
    }

    fn from_parts(
        lockfile_mode: LockfileMode,
        prior_facts: serde_json::Value,
        workspace_lockfile_facts: serde_json::Value,
        workspace_lockfile_facts_present: bool,
        selected_cache: Option<SelectedExtensionCache>,
        selected_cache_identity: Option<SelectedExtensionCacheIdentity>,
    ) -> Arc<Self> {
        let identity_digest = Arc::from(
            module_extension_replay_inputs_identity_digest(
                lockfile_mode,
                &prior_facts,
                &workspace_lockfile_facts,
                workspace_lockfile_facts_present,
                &selected_cache_identity,
            )
            .as_str(),
        );
        Arc::new(Self {
            lockfile_mode,
            prior_facts,
            workspace_lockfile_facts,
            workspace_lockfile_facts_present,
            selected_cache,
            selected_cache_identity,
            identity_digest,
        })
    }

    fn identity_digest(&self) -> &str {
        &self.identity_digest
    }

    fn has_facts(&self) -> bool {
        self.workspace_lockfile_facts_present || self.prior_facts != empty_facts()
    }
}

fn selected_extension_cache_identity(
    source: crate::dice_graph::LockfileContentKind,
    cache: &SelectedExtensionCache,
) -> SelectedExtensionCacheIdentity {
    SelectedExtensionCacheIdentity {
        source,
        selected_key: cache.selected_key.clone(),
        repo_specs_digest: selected_extension_cache_repo_specs_digest(cache),
        recorded_inputs: cache.recorded_inputs.clone(),
        workspace_root: cache.workspace_root.clone(),
        repo_env: cache.repo_env.clone(),
        repo_mappings: cache.repo_mappings.clone(),
    }
}

fn selected_extension_cache_repo_specs_digest(cache: &SelectedExtensionCache) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut repo_specs: Vec<_> = cache.repo_specs.iter().collect();
    repo_specs.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (name, spec) in repo_specs {
        name.hash(&mut hasher);
        let spec_json = serde_json::to_string(spec).unwrap_or_else(|_| format!("{spec:?}"));
        spec_json.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

fn facts_identity(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn module_extension_replay_inputs_identity_digest(
    lockfile_mode: LockfileMode,
    prior_facts: &serde_json::Value,
    workspace_lockfile_facts: &serde_json::Value,
    workspace_lockfile_facts_present: bool,
    selected_cache_identity: &Option<SelectedExtensionCacheIdentity>,
) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    lockfile_mode.hash(&mut hasher);
    facts_identity(prior_facts).hash(&mut hasher);
    facts_identity(workspace_lockfile_facts).hash(&mut hasher);
    workspace_lockfile_facts_present.hash(&mut hasher);
    selected_cache_identity.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("ModuleExtensionReplayInputsKey({})", extension_id)]
pub struct ModuleExtensionReplayInputsKey {
    pub workspace_id: crate::WorkspaceId,
    pub extension_id: Arc<str>,
    pub bzl_transitive_digest: Arc<str>,
    pub usages_digest: Arc<str>,
    pub project_root: Option<Arc<PathBuf>>,
    pub root_module_name: Arc<str>,
    pub repo_env: Arc<BTreeMap<String, String>>,
    pub repo_mappings: Arc<RepoMappingSnapshot>,
    pub repo_mapping_overrides: Arc<RepoMappingOverrides>,
}

#[async_trait]
impl Key for ModuleExtensionReplayInputsKey {
    type Value = slug_error::Result<Arc<ModuleExtensionReplayInputsValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let lockfile_inputs = ctx
            .compute(&BzlmodLockfileInputsKey::for_workspace_id(
                self.workspace_id.clone(),
            ))
            .await??;
        ModuleExtensionReplayInputsValue::from_lockfile_inputs(
            &self.extension_id,
            &self.bzl_transitive_digest,
            &self.usages_digest,
            self.project_root.as_deref().map(|path| path.as_path()),
            &self.root_module_name,
            self.repo_env.as_ref(),
            self.repo_mappings.as_ref(),
            self.repo_mapping_overrides.as_ref(),
            lockfile_inputs.as_ref(),
        )
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.identity_digest() == y.identity_digest(),
            _ => false,
        }
    }
}

fn extension_id_for_canonical_repo<'a>(
    aggregations: &'a BzlmodExtensionAggregationsDataValue,
    root_module_name: &str,
    canonical_name: &str,
) -> Option<&'a str> {
    let (owner_module, extension_name, _) = crate::parse_canonical_name(canonical_name)?;
    let mut matches = aggregations
        .extension_aggregations
        .iter()
        .filter_map(|(extension_id, aggregation)| {
            if aggregation.extension_name != extension_name {
                return None;
            }
            let owner = extract_owning_module(extension_id, root_module_name);
            owning_module_matches(owner_module, &owner).then_some(extension_id.as_str())
        })
        .collect::<Vec<_>>();
    matches.sort_unstable();
    if matches.len() > 1 {
        tracing::warn!(
            "Multiple extensions match canonical repo '{}'; choosing '{}' from {:?}",
            canonical_name,
            matches[0],
            matches
        );
    }
    matches.first().copied()
}

fn owning_module_matches(canonical_owner: &str, extension_owner: &str) -> bool {
    canonical_owner == extension_owner
        || (!canonical_owner.ends_with('+') && extension_owner == format!("{canonical_owner}+"))
        || (canonical_owner.ends_with('+')
            && extension_owner.strip_suffix('+') == Some(canonical_owner.trim_end_matches('+')))
}

fn ensure_extension_aggregations_data_workspace(
    workspace_id: &crate::WorkspaceId,
    aggregations: &BzlmodExtensionAggregationsDataValue,
    key_name: &str,
    subject: &str,
) -> slug_error::Result<()> {
    if aggregations.workspace_id != *workspace_id {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "{} for '{}' was computed with project root '{}', \
             but current bzlmod extension aggregation data root is '{}'",
            key_name,
            subject,
            workspace_id.canonical_project_root.display(),
            aggregations.workspace_id.canonical_project_root.display()
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct ExtensionSpokesIdentityValue {
    pub workspace_id: crate::WorkspaceId,
    pub extension_id: Arc<str>,
    pub bzl_transitive_digest: Arc<str>,
    pub usages_digest: Arc<str>,
    pub root_module_name: Arc<str>,
    #[allocative(skip)]
    pub(crate) aggregated: Arc<AggregatedExtension>,
    pub replay_inputs_identity_digest: Arc<str>,
    pub replay_inputs_have_facts: bool,
    pub repo_env_json: Arc<str>,
    pub repo_mappings_digest: Arc<str>,
    pub repo_mapping_overrides_digest: Arc<str>,
    pub repo_env: Arc<BTreeMap<String, String>>,
    pub repo_mappings: Arc<RepoMappingSnapshot>,
    pub repo_mapping_overrides: Arc<RepoMappingOverrides>,
}

async fn extension_spokes_identity_for_aggregation(
    ctx: &mut DiceComputations<'_>,
    workspace_id: &crate::WorkspaceId,
    aggregation: &BzlmodExtensionAggregationValue,
    bzl_transitive_digest: &ExtensionBzlTransitiveDigestValue,
) -> slug_error::Result<Arc<ExtensionSpokesIdentityValue>> {
    let repo_mappings =
        crate::bzlmod_repo_mappings_for_workspace_id(ctx, workspace_id.clone()).await?;
    let repo_env = ctx
        .compute(&BzlmodRepoEnvKey::for_workspace_id(workspace_id.clone()))
        .await??;
    let usages_digest = compute_extension_input_hash(aggregation.aggregated.as_ref());
    let replay_inputs = ctx
        .compute(&ModuleExtensionReplayInputsKey {
            workspace_id: workspace_id.clone(),
            extension_id: aggregation.extension_id.clone(),
            bzl_transitive_digest: Arc::from(bzl_transitive_digest.digest()),
            usages_digest: Arc::from(usages_digest.as_str()),
            project_root: Some(Arc::new(
                workspace_id.canonical_project_root.as_ref().clone(),
            )),
            root_module_name: aggregation.root_module_name.clone(),
            repo_env: repo_env.clone(),
            repo_mappings: repo_mappings.repo_mappings.clone(),
            repo_mapping_overrides: repo_mappings.repo_mapping_overrides.clone(),
        })
        .await??;

    Ok(Arc::new(ExtensionSpokesIdentityValue {
        workspace_id: workspace_id.clone(),
        extension_id: aggregation.extension_id.clone(),
        bzl_transitive_digest: Arc::from(bzl_transitive_digest.digest()),
        usages_digest: Arc::from(usages_digest.as_str()),
        root_module_name: aggregation.root_module_name.clone(),
        aggregated: aggregation.aggregated.clone(),
        replay_inputs_identity_digest: Arc::from(replay_inputs.identity_digest()),
        replay_inputs_have_facts: replay_inputs.has_facts(),
        repo_env_json: Arc::from(stable_json_string(repo_env.as_ref(), "{}").as_str()),
        repo_mappings_digest: Arc::from(
            repo_mappings_identity_digest(repo_mappings.repo_mappings.as_ref()).as_str(),
        ),
        repo_mapping_overrides_digest: Arc::from(
            repo_mapping_overrides_identity_digest(repo_mappings.repo_mapping_overrides.as_ref())
                .as_str(),
        ),
        repo_env,
        repo_mappings: repo_mappings.repo_mappings.clone(),
        repo_mapping_overrides: repo_mappings.repo_mapping_overrides.clone(),
    }))
}

pub async fn extension_spokes_identity_for_workspace(
    ctx: &mut DiceComputations<'_>,
    workspace_id: &crate::WorkspaceId,
    extension_id: &str,
) -> slug_error::Result<Option<Arc<ExtensionSpokesIdentityValue>>> {
    let aggregation = ctx
        .compute(&BzlmodExtensionAggregationKey {
            workspace_id: workspace_id.clone(),
            extension_id: Arc::from(extension_id),
        })
        .await??;
    let Some(aggregation) = aggregation else {
        return Ok(None);
    };
    let bzl_transitive_digest = ctx
        .compute(&ExtensionBzlTransitiveDigestKey {
            workspace_id: workspace_id.clone(),
            extension_id: Arc::from(extension_id),
            allow_missing_loads: false,
        })
        .await??;
    Ok(Some(
        extension_spokes_identity_for_aggregation(
            ctx,
            workspace_id,
            aggregation.as_ref(),
            bzl_transitive_digest.as_ref(),
        )
        .await?,
    ))
}

async fn extension_spokes_key_for_aggregation(
    ctx: &mut DiceComputations<'_>,
    workspace_id: &crate::WorkspaceId,
    aggregation: &BzlmodExtensionAggregationValue,
    bzl_transitive_digest: &ExtensionBzlTransitiveDigestValue,
) -> slug_error::Result<ExtensionSpokesKey> {
    let identity = extension_spokes_identity_for_aggregation(
        ctx,
        workspace_id,
        aggregation,
        bzl_transitive_digest,
    )
    .await?;
    Ok(ExtensionSpokesKey::for_workspace_id_with_inputs(
        workspace_id.clone(),
        identity.extension_id.as_ref(),
        identity.bzl_transitive_digest.as_ref(),
        identity.usages_digest.as_ref(),
        identity.root_module_name.as_ref(),
        identity.replay_inputs_identity_digest.as_ref(),
        identity.aggregated.clone(),
        identity.repo_env.clone(),
        identity.repo_mappings.clone(),
        identity.repo_mapping_overrides.clone(),
    ))
}

#[async_trait]
impl Key for BzlmodExtensionAggregationKey {
    type Value = slug_error::Result<Option<Arc<BzlmodExtensionAggregationValue>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let aggregations = ctx.compute(&BzlmodExtensionAggregationsDataKey).await?;
        ensure_extension_aggregations_data_workspace(
            &self.workspace_id,
            aggregations.as_ref(),
            "BzlmodExtensionAggregationKey",
            self.extension_id.as_ref(),
        )?;
        let Some(aggregated) = aggregations
            .extension_aggregations
            .get(self.extension_id.as_ref())
        else {
            return Ok(None);
        };
        Ok(Some(Arc::new(BzlmodExtensionAggregationValue {
            workspace_id: aggregations.workspace_id.clone(),
            extension_id: self.extension_id.clone(),
            aggregated: Arc::new(aggregated.clone()),
            root_module_name: Arc::from(aggregations.root_module_name.as_str()),
        })))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        false
    }
}

#[async_trait]
impl Key for ExtensionBzlTransitiveDigestKey {
    type Value = slug_error::Result<Arc<ExtensionBzlTransitiveDigestValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let aggregation = ctx
            .compute(&BzlmodExtensionAggregationKey {
                workspace_id: self.workspace_id.clone(),
                extension_id: self.extension_id.clone(),
            })
            .await??;
        let Some(aggregation) = aggregation else {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Extension '{}' not found while computing loaded .bzl digest",
                self.extension_id
            ));
        };
        let executor = MODULE_EXTENSION_EXECUTOR_IMPL.get().map_err(|e| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "ExtensionBzlTransitiveDigestKey requires the module extension executor: {}",
                e
            )
        })?;
        let digest = executor
            .extension_bzl_transitive_digest(
                ctx,
                self.extension_id.as_ref(),
                aggregation.aggregated.as_ref(),
                self.allow_missing_loads,
            )
            .await
            .map_err(|e| {
                record_bzlmod_event(
                    BzlmodEventKind::ExtensionReplayMissReason,
                    format!("{}:loaded_bzl_digest_error", self.extension_id),
                );
                e
            })?;
        Ok(Arc::new(ExtensionBzlTransitiveDigestValue::new(digest)))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        false
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct ExtensionBzlTransitiveDigestValue {
    digest: Arc<str>,
}

impl ExtensionBzlTransitiveDigestValue {
    fn new(digest: String) -> Self {
        Self {
            digest: Arc::from(digest),
        }
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[async_trait]
impl Key for ExtensionSpokesByExtensionIdKey {
    type Value = slug_error::Result<Option<Arc<ExtensionSpokesValue>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let aggregation = ctx
            .compute(&BzlmodExtensionAggregationKey {
                workspace_id: self.workspace_id.clone(),
                extension_id: self.extension_id.clone(),
            })
            .await??;
        let Some(aggregation) = aggregation else {
            return Ok(None);
        };
        let bzl_transitive_digest = ctx
            .compute(&ExtensionBzlTransitiveDigestKey {
                workspace_id: self.workspace_id.clone(),
                extension_id: self.extension_id.clone(),
                allow_missing_loads: false,
            })
            .await??;
        let spokes_key = extension_spokes_key_for_aggregation(
            ctx,
            &self.workspace_id,
            aggregation.as_ref(),
            bzl_transitive_digest.as_ref(),
        )
        .await?;

        match ctx.compute(&spokes_key).await {
            Ok(Ok(spokes)) => Ok(Some(spokes)),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "DICE compute failed for extension '{}' spokes: {}",
                self.extension_id,
                e
            )),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        false
    }
}

#[async_trait]
impl Key for ExtensionIdByCanonicalRepoKey {
    type Value = slug_error::Result<Option<Arc<str>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let aggregations = ctx.compute(&BzlmodExtensionAggregationsDataKey).await?;
        ensure_extension_aggregations_data_workspace(
            &self.workspace_id,
            aggregations.as_ref(),
            "ExtensionIdByCanonicalRepoKey",
            self.canonical_name.as_ref(),
        )?;
        if aggregations.extension_aggregations.is_empty() {
            return Ok(None);
        }
        Ok(extension_id_for_canonical_repo(
            aggregations.as_ref(),
            &aggregations.root_module_name,
            &self.canonical_name,
        )
        .map(Arc::from))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
}

#[async_trait]
impl Key for ExtensionSpokesByCanonicalRepoKey {
    type Value = slug_error::Result<Option<Arc<ExtensionSpokesValue>>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let extension_id = ctx
            .compute(&ExtensionIdByCanonicalRepoKey {
                workspace_id: self.workspace_id.clone(),
                canonical_name: self.canonical_name.clone(),
            })
            .await??;
        let Some(extension_id) = extension_id else {
            return Ok(None);
        };
        let bzl_transitive_digest = ctx
            .compute(&ExtensionBzlTransitiveDigestKey {
                workspace_id: self.workspace_id.clone(),
                extension_id: extension_id.clone(),
                allow_missing_loads: false,
            })
            .await??;
        let aggregation = ctx
            .compute(&BzlmodExtensionAggregationKey {
                workspace_id: self.workspace_id.clone(),
                extension_id,
            })
            .await??;
        let Some(aggregation) = aggregation else {
            return Ok(None);
        };
        let spokes_key = extension_spokes_key_for_aggregation(
            ctx,
            &self.workspace_id,
            aggregation.as_ref(),
            bzl_transitive_digest.as_ref(),
        )
        .await?;

        match ctx.compute(&spokes_key).await {
            Ok(Ok(spokes)) => Ok(Some(spokes)),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "DICE compute failed for canonical repo '{}' spokes: {}",
                self.canonical_name,
                e
            )),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        false
    }
}

#[async_trait]
impl Key for ExtensionSpokesKey {
    type Value = slug_error::Result<Arc<ExtensionSpokesValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let aggregation = BzlmodExtensionAggregationValue {
            workspace_id: self.workspace_id.clone(),
            extension_id: self.extension_id.clone(),
            aggregated: self.aggregated.clone(),
            root_module_name: self.root_module_name.clone(),
        };
        let usages_digest = compute_extension_input_hash(aggregation.aggregated.as_ref());
        if !self.usages_digest.is_empty() && self.usages_digest.as_ref() != usages_digest {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "Extension '{}' aggregation digest changed while computing generated repo spokes: expected {}, got {}",
                self.extension_id,
                self.usages_digest,
                usages_digest
            ));
        }
        let replay_inputs = ctx
            .compute(&ModuleExtensionReplayInputsKey {
                workspace_id: self.workspace_id.clone(),
                extension_id: self.extension_id.clone(),
                bzl_transitive_digest: self.bzl_transitive_digest.clone(),
                usages_digest: Arc::from(usages_digest.as_str()),
                project_root: Some(Arc::new(
                    self.workspace_id.canonical_project_root.as_ref().clone(),
                )),
                root_module_name: self.root_module_name.clone(),
                repo_env: self.repo_env.clone(),
                repo_mappings: self.repo_mappings.clone(),
                repo_mapping_overrides: self.repo_mapping_overrides.clone(),
            })
            .await??;
        if !self.replay_inputs_identity_digest.is_empty()
            && self.replay_inputs_identity_digest.as_ref() != replay_inputs.identity_digest()
        {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "Extension '{}' replay input digest changed while computing generated repo spokes: expected {}, got {}",
                self.extension_id,
                self.replay_inputs_identity_digest,
                replay_inputs.identity_digest()
            ));
        }
        let extension_key = create_extension_execution_key_from_aggregation(
            &aggregation,
            self.repo_env.as_ref(),
            replay_inputs,
            self.repo_mappings.as_ref(),
            self.repo_mapping_overrides.as_ref(),
            self.bzl_transitive_digest.clone(),
        );
        record_bzlmod_event(
            BzlmodEventKind::ExtensionSpokesCompute,
            self.extension_id.as_ref(),
        );
        let result = ctx.compute(&extension_key).await??;
        let mut spokes = BTreeMap::new();
        for (internal_name, repo_spec) in result.generated_repo_specs.iter() {
            let Some(canonical_name) = result.canonical_name(internal_name) else {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "Extension '{}' generated repo '{}' without a canonical name",
                    self.extension_id,
                    internal_name
                ));
            };
            let repo_spec_json = serde_json::to_string(repo_spec).map_err(|e| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Input,
                    "Failed to serialize RepoSpec for extension '{}' repo '{}': {}",
                    self.extension_id,
                    internal_name,
                    e
                )
            })?;
            let spec_hash = repo_spec.compute_hash();
            spokes.insert(
                internal_name.clone(),
                ExtensionSpoke {
                    internal_name: Arc::from(internal_name.as_str()),
                    canonical_name: Arc::from(canonical_name),
                    spec_hash: Arc::from(spec_hash.as_str()),
                    repo_spec_json: Arc::from(repo_spec_json.as_str()),
                    repo_spec: Arc::new(repo_spec.clone()),
                },
            );
        }

        Ok(Arc::new(ExtensionSpokesValue {
            workspace_id: self.workspace_id.clone(),
            extension_id: self.extension_id.clone(),
            bzl_transitive_digest: self.bzl_transitive_digest.clone(),
            usages_digest: self.usages_digest.clone(),
            replay_inputs_identity_digest: self.replay_inputs_identity_digest.clone(),
            repo_mappings_digest: Arc::from(
                repo_mappings_identity_digest(self.repo_mappings.as_ref()).as_str(),
            ),
            repo_mapping_overrides_digest: Arc::from(
                repo_mapping_overrides_identity_digest(self.repo_mapping_overrides.as_ref())
                    .as_str(),
            ),
            project_root: self.workspace_id.canonical_project_root.clone(),
            repo_env: self.repo_env.clone(),
            spokes,
            recorded_inputs: Arc::new(result.recorded_inputs.clone()),
            recorded_input_workspace_root: result.recorded_input_context.workspace_root.clone(),
            recorded_input_repo_env: result.recorded_input_context.repo_env.clone(),
            recorded_input_repo_mappings: result.recorded_input_context.repo_mappings.clone(),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
}

/// Errors during module extension execution.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
pub enum ModuleExtensionError {
    #[error("Module extension execution failed for '{extension_id}': {reason}")]
    ExecutionFailed {
        extension_id: String,
        reason: String,
    },

    #[error(
        "Failed to create temporary working directory for extension '{extension_id}': {reason}"
    )]
    TempDirFailed {
        extension_id: String,
        reason: String,
    },

    #[error("Extension '{extension_id}' not found")]
    ExtensionNotFound { extension_id: String },

    #[error("Failed to load extension .bzl file: {path}")]
    BzlLoadFailed { path: String },

    #[error(
        "MODULE.bazel.lock is no longer up-to-date because {reason}. \
        Please run `bazel mod deps --lockfile_mode=update` to update your lockfile."
    )]
    OutdatedLockfile { reason: String },
}

/// Result of module extension evaluation.
///
/// Contains captured RepoSpecs but NO materialized repositories.
/// Repositories are created lazily when accessed during a build.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ModuleExtensionResult {
    /// Extension identifier: "@@module//path:file.bzl%extension_name"
    pub extension_id: Arc<str>,

    /// Hash of extension inputs (tags from all modules) for cache invalidation.
    pub input_hash: String,

    /// Generated repository specifications (NOT materialized).
    /// Keys are internal names (e.g., "numpy"), values are RepoSpecs.
    ///
    /// `FxHashMap` so iteration is stable across invocations (Plan 21.2).
    pub generated_repo_specs: FxHashMap<String, RepoSpec>,

    /// Canonical name mapping.
    /// Maps internal_name -> canonical_name (e.g., "numpy" -> "_main+pip+numpy")
    pub canonical_names: FxHashMap<String, String>,

    /// Metadata returned by module_ctx.extension_metadata(...).
    ///
    /// Bazel stores facts as part of `SingleExtensionValue` and deliberately
    /// excludes them from normal replay invalidation.
    #[allocative(skip)]
    pub metadata: ModuleExtensionMetadata,

    /// Recorded inputs that affect extension execution.
    pub recorded_inputs: Vec<String>,

    /// Current-command context needed to validate recorded inputs for DICE value
    /// reuse after fresh extension execution.
    #[allocative(skip)]
    recorded_input_context: ModuleExtensionRecordedInputContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
struct ModuleExtensionRecordedInputContext {
    #[allocative(skip)]
    workspace_root: Option<Arc<PathBuf>>,
    #[allocative(skip)]
    repo_env: Arc<BTreeMap<String, String>>,
    #[allocative(skip)]
    repo_mappings: Arc<RepoMappingSnapshot>,
}

impl ModuleExtensionRecordedInputContext {
    #[cfg(test)]
    fn empty() -> Self {
        Self {
            workspace_root: None,
            repo_env: Arc::new(BTreeMap::new()),
            repo_mappings: Arc::new(RepoMappingSnapshot::new()),
        }
    }

    fn new(
        workspace_root: Option<PathBuf>,
        repo_env: BTreeMap<String, String>,
        repo_mappings: RepoMappingSnapshot,
    ) -> Self {
        Self {
            workspace_root: workspace_root.map(Arc::new),
            repo_env: Arc::new(repo_env),
            repo_mappings: Arc::new(repo_mappings),
        }
    }

    fn from_selected_cache(selected: &SelectedExtensionCache) -> Self {
        Self::new(
            selected.workspace_root.clone(),
            selected.repo_env.clone().unwrap_or_default(),
            selected.repo_mappings.clone().unwrap_or_default(),
        )
    }
}

impl ModuleExtensionResult {
    /// Create a new extension result.
    ///
    /// `root_module_name` is the name of the root module (from MODULE.bazel
    /// `module(name=…)`). It is required so canonical names use Bazel's
    /// `_main` placeholder for the root module's own extensions; without it
    /// the root module's declared name leaks into canonical names and they
    /// disagree with the cells pre-computed in `pending_repo_cells.rs`.
    #[cfg(test)]
    pub fn new(
        extension_id: Arc<str>,
        input_hash: String,
        generated_repo_specs: FxHashMap<String, RepoSpec>,
        root_module_name: &str,
    ) -> Self {
        Self::new_with_metadata(
            extension_id,
            input_hash,
            generated_repo_specs,
            root_module_name,
            ModuleExtensionMetadata::default(),
            Vec::new(),
        )
    }

    #[cfg(test)]
    pub fn new_with_metadata(
        extension_id: Arc<str>,
        input_hash: String,
        generated_repo_specs: FxHashMap<String, RepoSpec>,
        root_module_name: &str,
        metadata: ModuleExtensionMetadata,
        recorded_inputs: Vec<String>,
    ) -> Self {
        Self::new_with_metadata_and_recorded_input_context(
            extension_id,
            input_hash,
            generated_repo_specs,
            root_module_name,
            metadata,
            recorded_inputs,
            ModuleExtensionRecordedInputContext::empty(),
        )
    }

    fn new_with_metadata_and_recorded_input_context(
        extension_id: Arc<str>,
        input_hash: String,
        generated_repo_specs: FxHashMap<String, RepoSpec>,
        root_module_name: &str,
        metadata: ModuleExtensionMetadata,
        recorded_inputs: Vec<String>,
        recorded_input_context: ModuleExtensionRecordedInputContext,
    ) -> Self {
        let canonical_names =
            build_canonical_names(&extension_id, &generated_repo_specs, root_module_name);
        Self {
            extension_id,
            input_hash,
            generated_repo_specs,
            canonical_names,
            metadata,
            recorded_inputs,
            recorded_input_context,
        }
    }

    /// Get the canonical name for a repository by its internal name.
    pub fn canonical_name(&self, internal_name: &str) -> Option<&str> {
        self.canonical_names.get(internal_name).map(|s| s.as_str())
    }

    /// Get a RepoSpec by internal name.
    pub fn get_repo_spec(&self, internal_name: &str) -> Option<&RepoSpec> {
        self.generated_repo_specs.get(internal_name)
    }

    /// Get all internal repository names.
    pub fn repo_names(&self) -> impl Iterator<Item = &str> {
        self.generated_repo_specs.keys().map(|s| s.as_str())
    }

    /// Check if this result contains a repository with the given internal name.
    pub fn contains_repo(&self, internal_name: &str) -> bool {
        self.generated_repo_specs.contains_key(internal_name)
    }

    /// Get the number of generated repositories.
    pub fn repo_count(&self) -> usize {
        self.generated_repo_specs.len()
    }

    /// Look up internal name from canonical name.
    pub fn internal_name_from_canonical(&self, canonical: &str) -> Option<&str> {
        self.canonical_names
            .iter()
            .find(|(_, c)| c.as_str() == canonical)
            .map(|(i, _)| i.as_str())
    }
}

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display("ModuleExtensionRecordedInputsKey({})", recorded_inputs.len())]
pub struct ModuleExtensionRecordedInputsKey {
    workspace_id: crate::WorkspaceId,
    recorded_inputs: Arc<Vec<String>>,
    workspace_root: Option<Arc<PathBuf>>,
    repo_env: Option<Arc<BTreeMap<String, String>>>,
    repo_mappings: Option<Arc<RepoMappingSnapshot>>,
}

impl ModuleExtensionRecordedInputsKey {
    pub fn for_workspace_id(
        recorded_inputs: Vec<String>,
        workspace_id: crate::WorkspaceId,
        repo_env: Arc<BTreeMap<String, String>>,
        repo_mappings: Arc<RepoMappingSnapshot>,
    ) -> Self {
        Self {
            workspace_root: Some(workspace_id.canonical_project_root.clone()),
            workspace_id,
            recorded_inputs: Arc::new(recorded_inputs),
            repo_env: Some(repo_env),
            repo_mappings: Some(repo_mappings),
        }
    }

    #[cfg(test)]
    pub fn new(
        recorded_inputs: Vec<String>,
        workspace_root: Option<Arc<PathBuf>>,
        repo_env: Option<Arc<BTreeMap<String, String>>>,
        repo_mappings: Option<Arc<RepoMappingSnapshot>>,
    ) -> Self {
        Self {
            workspace_id: workspace_root
                .as_deref()
                .map(|root| crate::WorkspaceId::new(root.clone(), root.join("buck-out/v2")))
                .unwrap_or_else(crate::WorkspaceId::no_project_sentinel),
            recorded_inputs: Arc::new(recorded_inputs),
            workspace_root,
            repo_env,
            repo_mappings,
        }
    }

    fn from_selected_cache_for_workspace_id(
        workspace_id: crate::WorkspaceId,
        selected: &SelectedExtensionCache,
    ) -> Self {
        Self {
            workspace_root: Some(workspace_id.canonical_project_root.clone()),
            workspace_id,
            recorded_inputs: Arc::new(selected.recorded_inputs.clone()),
            repo_env: selected.repo_env.clone().map(Arc::new),
            repo_mappings: selected.repo_mappings.clone().map(Arc::new),
        }
    }
}

impl Dupe for ModuleExtensionRecordedInputsKey {
    fn dupe(&self) -> Self {
        Self {
            workspace_id: self.workspace_id.clone(),
            recorded_inputs: self.recorded_inputs.clone(),
            workspace_root: self.workspace_root.clone(),
            repo_env: self.repo_env.clone(),
            repo_mappings: self.repo_mappings.clone(),
        }
    }
}

#[async_trait]
impl Key for ModuleExtensionRecordedInputsKey {
    type Value = Result<(), Arc<str>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if let Ok(reader) = REPOSITORY_MATERIALIZATION_STATE_READER_IMPL.get() {
            return validate_recorded_inputs_with_dice_reader(
                ctx,
                *reader,
                self.workspace_id.clone(),
                self.recorded_inputs.as_slice(),
                self.repo_env.as_deref(),
                self.repo_mappings.as_deref(),
            )
            .await;
        }

        #[cfg(not(test))]
        {
            return Err(Arc::from("recorded_inputs_reader_unavailable"));
        }

        #[cfg(test)]
        {
            crate::lockfile::validate_recorded_inputs_current(
                self.recorded_inputs.as_slice(),
                self.workspace_root.as_deref().map(PathBuf::as_path),
                self.repo_env.as_deref(),
                self.repo_mappings.as_deref(),
            )
            .map_err(Arc::from)
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(_x: &Self::Value) -> bool {
        false
    }
}

/// DICE key for module extension evaluation.
///
/// When computed, this:
/// 1. Checks lockfile for cached result (if project_root is set)
/// 2. Creates a temporary working directory for module_ctx
/// 3. Loads the extension's .bzl file
/// 4. Builds module_ctx from aggregated tags
/// 5. Executes implementation(module_ctx) with RepoSpec capture
/// 6. Cleans up the temporary directory
/// 7. Returns ModuleExtensionResult with captured specs
///
/// Note: NO downloads or repository materialization happens during this computation.
/// Repositories are materialized lazily via `ExtensionRepoExecutionKey`.
///
/// Note: Hash and Eq are implemented manually because `AggregatedExtension` contains
/// HashMap. The `input_hash` field is used for hashing, ensuring deterministic cache behavior.
/// `project_root` is included because it identifies the workspace whose lockfile,
/// local `.bzl` loads, and generated repo namespace are being evaluated.
#[derive(Clone, Debug, Display, Allocative)]
#[display(
    "ModuleExtensionKey({}, {}, {})",
    extension_id,
    input_hash,
    bzl_transitive_digest
)]
pub struct ModuleExtensionExecutionKey {
    /// Extension identifier: "@@module//path:file.bzl%extension_name"
    pub extension_id: Arc<str>,

    /// Hash of input tags for cache invalidation.
    /// This hash covers all tags from all modules that use this extension.
    pub input_hash: Arc<str>,

    /// Digest of the extension implementation's loaded `.bzl` graph.
    ///
    /// Bazel's `SingleExtensionEvalFunction` keys extension replay by the
    /// runnable extension's transitive `.bzl` digest. Slug's current digest is
    /// still an approximation for project-local literal loads, but it must be
    /// part of the key identity so a DICE hit cannot skip replay invalidation.
    pub bzl_transitive_digest: Arc<str>,

    /// Aggregated extension data from all modules.
    /// Contains all the tags needed to build module_ctx.
    pub aggregated: Arc<AggregatedExtension>,

    /// Root module name (needed for build_module_context).
    pub root_module_name: Arc<str>,

    /// Project root for read-only lockfile access (optional).
    /// If set, Bazel-authored lockfile caches may be read. Ordinary builds
    /// must not write `MODULE.bazel.lock`; it is a Bazel-owned compatibility
    /// surface, not a Slug-private extension cache.
    pub project_root: Option<Arc<PathBuf>>,

    /// Workspace identity for DICE lookups triggered while executing the
    /// extension. This carries the exact workspace/output-base identity from
    /// the parent DICE key instead of re-deriving it from project root.
    pub workspace_id: crate::WorkspaceId,

    /// DICE-owned replay inputs selected from the visible/hidden lockfiles.
    ///
    /// Lockfile cache/facts selection must happen before extension execution so
    /// a DICE hit cannot skip replay invalidation or reopen lockfiles here.
    pub replay_inputs: Arc<ModuleExtensionReplayInputsValue>,

    /// Effective Bazel repository environment used for ENV recorded-input replay.
    pub repo_env: Arc<BTreeMap<String, String>>,

    /// Current scoped repository mappings used for REPO_MAPPING recorded-input replay.
    pub repo_mappings: Arc<RepoMappingSnapshot>,

    /// Root-module override_repo rows used for extension-generated repo mappings.
    pub repo_mapping_overrides: Arc<RepoMappingOverrides>,
}

impl std::hash::Hash for ModuleExtensionExecutionKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the identifying fields; input_hash represents the aggregated data
        self.extension_id.hash(state);
        self.input_hash.hash(state);
        self.bzl_transitive_digest.hash(state);
        self.root_module_name.hash(state);
        self.project_root.hash(state);
        self.workspace_id.hash(state);
        self.replay_inputs.identity_digest().hash(state);
        self.repo_env.hash(state);
        self.repo_mappings.hash(state);
        self.repo_mapping_overrides.hash(state);
    }
}

impl PartialEq for ModuleExtensionExecutionKey {
    fn eq(&self, other: &Self) -> bool {
        // Compare by identifying fields; input_hash represents the aggregated data
        self.extension_id == other.extension_id
            && self.input_hash == other.input_hash
            && self.bzl_transitive_digest == other.bzl_transitive_digest
            && self.root_module_name == other.root_module_name
            && self.project_root == other.project_root
            && self.workspace_id == other.workspace_id
            && self.replay_inputs.identity_digest() == other.replay_inputs.identity_digest()
            && self.repo_env == other.repo_env
            && self.repo_mappings == other.repo_mappings
            && self.repo_mapping_overrides == other.repo_mapping_overrides
    }
}

impl Eq for ModuleExtensionExecutionKey {}

// Manual Dupe implementation
impl Dupe for ModuleExtensionExecutionKey {
    fn dupe(&self) -> Self {
        Self {
            extension_id: self.extension_id.dupe(),
            input_hash: self.input_hash.dupe(),
            bzl_transitive_digest: self.bzl_transitive_digest.dupe(),
            aggregated: self.aggregated.dupe(),
            root_module_name: self.root_module_name.dupe(),
            project_root: self.project_root.clone(),
            workspace_id: self.workspace_id.clone(),
            replay_inputs: self.replay_inputs.clone(),
            repo_env: self.repo_env.clone(),
            repo_mappings: self.repo_mappings.clone(),
            repo_mapping_overrides: self.repo_mapping_overrides.clone(),
        }
    }
}

impl ModuleExtensionExecutionKey {
    /// Create a new extension execution key from aggregated extension data.
    #[cfg(test)]
    fn new(aggregated: AggregatedExtension, root_module_name: String) -> Self {
        let extension_id = Arc::from(aggregated.extension_id.as_str());
        let input_hash = Arc::from(compute_extension_input_hash(&aggregated).as_str());
        let bzl_transitive_digest =
            Arc::from(compute_bzl_transitive_digest(&extension_id).as_str());
        Self {
            extension_id,
            input_hash,
            bzl_transitive_digest,
            aggregated: Arc::new(aggregated),
            root_module_name: Arc::from(root_module_name.as_str()),
            project_root: None,
            workspace_id: crate::WorkspaceId::for_project_root(PathBuf::from("__test__")),
            replay_inputs: ModuleExtensionReplayInputsValue::empty(LockfileMode::Update),
            repo_env: Arc::new(BTreeMap::new()),
            repo_mappings: Arc::new(RepoMappingSnapshot::new()),
            repo_mapping_overrides: Arc::new(RepoMappingOverrides::new()),
        }
    }

    /// Create a new extension execution key with lockfile support.
    #[cfg(test)]
    fn new_with_lockfile(
        aggregated: AggregatedExtension,
        root_module_name: String,
        project_root: PathBuf,
        hidden_lockfile_path: Option<PathBuf>,
        visible_lockfile_digest: Option<String>,
        hidden_lockfile_digest: Option<String>,
        lockfile_mode: LockfileMode,
        repo_env: BTreeMap<String, String>,
        repo_mappings: RepoMappingSnapshot,
        repo_mapping_overrides: RepoMappingOverrides,
    ) -> Self {
        let hidden_lockfile = hidden_lockfile_path.as_ref().map(|path| {
            Arc::new(LockfileContentValue {
                path: Arc::new(path.clone()),
                digest: hidden_lockfile_digest.clone(),
                tracked_by_dice: true,
                lockfile: None,
            })
        });
        Self::new_with_tracked_lockfiles(
            aggregated,
            root_module_name,
            project_root,
            visible_lockfile_digest,
            hidden_lockfile_digest,
            None,
            hidden_lockfile,
            lockfile_mode,
            repo_env,
            repo_mappings,
            repo_mapping_overrides,
        )
    }

    /// Create a new extension execution key with DICE-tracked lockfile values.
    #[cfg(test)]
    fn new_with_tracked_lockfiles(
        aggregated: AggregatedExtension,
        root_module_name: String,
        project_root: PathBuf,
        visible_lockfile_digest: Option<String>,
        hidden_lockfile_digest: Option<String>,
        visible_lockfile: Option<Arc<LockfileContentValue>>,
        hidden_lockfile: Option<Arc<LockfileContentValue>>,
        lockfile_mode: LockfileMode,
        repo_env: BTreeMap<String, String>,
        repo_mappings: RepoMappingSnapshot,
        repo_mapping_overrides: RepoMappingOverrides,
    ) -> Self {
        let extension_id = Arc::from(aggregated.extension_id.as_str());
        let workspace_id = crate::WorkspaceId::for_project_root(project_root.clone());
        let bzl_transitive_digest = Arc::from(
            compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
                &extension_id,
                project_root.as_path(),
                Some(&repo_mappings),
            )
            .as_str(),
        );
        Self::new_with_tracked_lockfiles_and_bzl_digest(
            aggregated,
            root_module_name,
            project_root,
            visible_lockfile_digest,
            hidden_lockfile_digest,
            visible_lockfile,
            hidden_lockfile,
            lockfile_mode,
            repo_env,
            repo_mappings,
            repo_mapping_overrides,
            bzl_transitive_digest,
            workspace_id,
        )
    }

    #[cfg(test)]
    fn new_with_tracked_lockfiles_and_bzl_digest(
        aggregated: AggregatedExtension,
        root_module_name: String,
        project_root: PathBuf,
        visible_lockfile_digest: Option<String>,
        hidden_lockfile_digest: Option<String>,
        visible_lockfile: Option<Arc<LockfileContentValue>>,
        hidden_lockfile: Option<Arc<LockfileContentValue>>,
        lockfile_mode: LockfileMode,
        repo_env: BTreeMap<String, String>,
        repo_mappings: RepoMappingSnapshot,
        repo_mapping_overrides: RepoMappingOverrides,
        bzl_transitive_digest: Arc<str>,
        workspace_id: crate::WorkspaceId,
    ) -> Self {
        let lockfile_inputs = BzlmodLockfileInputsValue::from_values(
            hidden_lockfile
                .as_ref()
                .map(|value| value.path.as_ref().clone()),
            visible_lockfile,
            hidden_lockfile,
            lockfile_mode,
        );
        let mut lockfile_inputs = lockfile_inputs;
        lockfile_inputs.visible_lockfile_digest = visible_lockfile_digest;
        lockfile_inputs.hidden_lockfile_digest = hidden_lockfile_digest;
        let replay_inputs = ModuleExtensionReplayInputsValue::from_lockfile_inputs(
            aggregated.extension_id.as_str(),
            &bzl_transitive_digest,
            compute_extension_input_hash(&aggregated).as_str(),
            Some(project_root.as_path()),
            &root_module_name,
            &repo_env,
            &repo_mappings,
            &repo_mapping_overrides,
            &lockfile_inputs,
        )
        .unwrap_or_else(|_| ModuleExtensionReplayInputsValue::empty(lockfile_mode));
        Self::new_with_replay_inputs_and_bzl_digest(
            aggregated,
            root_module_name,
            project_root,
            replay_inputs,
            repo_env,
            repo_mappings,
            repo_mapping_overrides,
            bzl_transitive_digest,
            workspace_id,
        )
    }

    fn new_with_replay_inputs_and_bzl_digest(
        aggregated: AggregatedExtension,
        root_module_name: String,
        project_root: PathBuf,
        replay_inputs: Arc<ModuleExtensionReplayInputsValue>,
        repo_env: BTreeMap<String, String>,
        repo_mappings: RepoMappingSnapshot,
        repo_mapping_overrides: RepoMappingOverrides,
        bzl_transitive_digest: Arc<str>,
        workspace_id: crate::WorkspaceId,
    ) -> Self {
        let extension_id = Arc::from(aggregated.extension_id.as_str());
        let input_hash = Arc::from(compute_extension_input_hash(&aggregated).as_str());
        Self {
            extension_id,
            input_hash,
            bzl_transitive_digest,
            aggregated: Arc::new(aggregated),
            root_module_name: Arc::from(root_module_name.as_str()),
            project_root: Some(Arc::new(project_root)),
            workspace_id,
            replay_inputs,
            repo_env: Arc::new(repo_env),
            repo_mappings: Arc::new(repo_mappings),
            repo_mapping_overrides: Arc::new(repo_mapping_overrides),
        }
    }

    /// Create from Arc references with lockfile support.
    #[cfg(test)]
    fn from_arcs_with_lockfile(
        extension_id: Arc<str>,
        input_hash: Arc<str>,
        aggregated: Arc<AggregatedExtension>,
        root_module_name: Arc<str>,
        project_root: Arc<PathBuf>,
        _visible_lockfile_digest: Option<Arc<str>>,
        _hidden_lockfile_digest: Option<Arc<str>>,
        lockfile_mode: LockfileMode,
        repo_env: Arc<BTreeMap<String, String>>,
        repo_mappings: Arc<RepoMappingSnapshot>,
        repo_mapping_overrides: Arc<RepoMappingOverrides>,
    ) -> Self {
        let workspace_id = crate::WorkspaceId::for_project_root(project_root.as_ref().clone());
        let bzl_transitive_digest = Arc::from(
            compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
                &extension_id,
                project_root.as_ref(),
                Some(repo_mappings.as_ref()),
            )
            .as_str(),
        );
        Self {
            extension_id,
            input_hash,
            bzl_transitive_digest,
            aggregated,
            root_module_name,
            project_root: Some(project_root),
            workspace_id,
            replay_inputs: ModuleExtensionReplayInputsValue::empty(lockfile_mode),
            repo_env,
            repo_mappings,
            repo_mapping_overrides,
        }
    }

    /// Create a minimal key (for testing or when aggregated data is not available).
    /// This is primarily for backward compatibility with tests.
    #[cfg(test)]
    fn new_minimal(extension_id: String, input_hash: String) -> Self {
        let bzl_transitive_digest = compute_bzl_transitive_digest(&extension_id);
        Self {
            extension_id: Arc::from(extension_id.as_str()),
            input_hash: Arc::from(input_hash.as_str()),
            bzl_transitive_digest: Arc::from(bzl_transitive_digest.as_str()),
            aggregated: Arc::new(AggregatedExtension::default()),
            root_module_name: Arc::from("_main"),
            project_root: None,
            workspace_id: crate::WorkspaceId::for_project_root(PathBuf::from("__test__")),
            replay_inputs: ModuleExtensionReplayInputsValue::empty(LockfileMode::Update),
            repo_env: Arc::new(BTreeMap::new()),
            repo_mappings: Arc::new(RepoMappingSnapshot::new()),
            repo_mapping_overrides: Arc::new(RepoMappingOverrides::new()),
        }
    }

    /// Get the aggregated extension data.
    pub fn aggregated(&self) -> &AggregatedExtension {
        &self.aggregated
    }

    /// Get the root module name.
    pub fn root_module_name(&self) -> &str {
        &self.root_module_name
    }

    /// Get the project root (if set for lockfile support).
    pub fn project_root(&self) -> Option<&PathBuf> {
        self.project_root.as_ref().map(|p| p.as_ref())
    }

    #[cfg(test)]
    fn execution_workspace_id(&self) -> crate::WorkspaceId {
        self.workspace_id.clone()
    }
}

fn empty_facts() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn facts_for_message(facts: &serde_json::Value) -> String {
    serde_json::to_string(facts).unwrap_or_else(|_| facts.to_string())
}

fn validate_error_mode_facts(
    extension_id: &str,
    lockfile_mode: LockfileMode,
    new_facts: &serde_json::Value,
    workspace_lockfile_facts: &serde_json::Value,
) -> Result<(), ModuleExtensionError> {
    if lockfile_mode == LockfileMode::Error && new_facts != workspace_lockfile_facts {
        return Err(ModuleExtensionError::OutdatedLockfile {
            reason: format!(
                "the extension '{}' has changed its facts: {} != {}",
                extension_id,
                facts_for_message(new_facts),
                facts_for_message(workspace_lockfile_facts),
            ),
        });
    }

    Ok(())
}

fn verify_observed_lockfile_digest(
    value: &LockfileContentValue,
    expected_digest: Option<&str>,
    label: &str,
) -> slug_error::Result<()> {
    if let Some(expected_digest) = expected_digest {
        let actual_digest = value.digest.as_deref().ok_or_else(|| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "{label} became unreadable while computing module extension: expected digest {}",
                expected_digest
            )
        })?;
        if actual_digest == expected_digest {
            return Ok(());
        }
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Input,
            "{label} changed while computing module extension: expected digest {}, got {} at {}",
            expected_digest,
            actual_digest,
            value.path.display()
        ));
    }
    Ok(())
}

pub(crate) async fn selected_cache_recorded_inputs_current(
    ctx: &mut DiceComputations<'_>,
    workspace_id: crate::WorkspaceId,
    extension_id: &str,
    selected: &SelectedExtensionCache,
) -> slug_error::Result<bool> {
    let key = ModuleExtensionRecordedInputsKey::from_selected_cache_for_workspace_id(
        workspace_id,
        selected,
    );
    match ctx.compute(&key).await? {
        Ok(()) => Ok(true),
        Err(reason) => {
            record_bzlmod_event(
                BzlmodEventKind::ExtensionReplayMissReason,
                format!("{extension_id}:{reason}"),
            );
            tracing::debug!(
                "Extension cache miss for '{}': recorded input validation failed ({})",
                extension_id,
                reason
            );
            Ok(false)
        }
    }
}

async fn validate_fresh_recorded_inputs_dependency(
    ctx: &mut DiceComputations<'_>,
    extension_id: &str,
    recorded_inputs: Vec<String>,
    workspace_id: crate::WorkspaceId,
    repo_env: Arc<BTreeMap<String, String>>,
    repo_mappings: Arc<RepoMappingSnapshot>,
) -> slug_error::Result<Vec<String>> {
    if recorded_inputs.is_empty() {
        return Ok(recorded_inputs);
    }
    let key = ModuleExtensionRecordedInputsKey::for_workspace_id(
        recorded_inputs,
        workspace_id,
        repo_env,
        repo_mappings,
    );
    match ctx.compute(&key).await {
        Ok(Ok(())) => Ok(key.recorded_inputs.as_ref().clone()),
        Ok(Err(reason)) => Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "Fresh module extension '{}' recorded stale input: {}",
            extension_id,
            reason
        )),
        Err(e) => Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "DICE compute failed while validating fresh recorded inputs for '{}': {}",
            extension_id,
            e
        )),
    }
}

#[derive(Clone, Debug, Display, Allocative)]
#[display("ModuleExtensionFreshEvalKey({})", extension_id)]
struct ModuleExtensionFreshEvalKey {
    extension_id: Arc<str>,
    input_hash: Arc<str>,
    bzl_transitive_digest: Arc<str>,
    aggregated: Arc<AggregatedExtension>,
    root_module_name: Arc<str>,
    project_root: Option<Arc<PathBuf>>,
    workspace_id: crate::WorkspaceId,
    #[allocative(skip)]
    prior_facts: Arc<serde_json::Value>,
    repo_env: Arc<BTreeMap<String, String>>,
    repo_mappings: Arc<RepoMappingSnapshot>,
    repo_mapping_overrides: Arc<RepoMappingOverrides>,
}

impl ModuleExtensionFreshEvalKey {
    fn from_execution_key(
        key: &ModuleExtensionExecutionKey,
        prior_facts: Arc<serde_json::Value>,
    ) -> Self {
        Self {
            extension_id: key.extension_id.clone(),
            input_hash: key.input_hash.clone(),
            bzl_transitive_digest: key.bzl_transitive_digest.clone(),
            aggregated: key.aggregated.clone(),
            root_module_name: key.root_module_name.clone(),
            project_root: key.project_root.clone(),
            workspace_id: key.workspace_id.clone(),
            prior_facts,
            repo_env: key.repo_env.clone(),
            repo_mappings: key.repo_mappings.clone(),
            repo_mapping_overrides: key.repo_mapping_overrides.clone(),
        }
    }
}

impl std::hash::Hash for ModuleExtensionFreshEvalKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.extension_id.hash(state);
        self.input_hash.hash(state);
        self.bzl_transitive_digest.hash(state);
        self.root_module_name.hash(state);
        self.project_root.hash(state);
        self.workspace_id.hash(state);
        facts_identity(&self.prior_facts).hash(state);
        self.repo_env.hash(state);
        self.repo_mappings.hash(state);
        self.repo_mapping_overrides.hash(state);
    }
}

impl PartialEq for ModuleExtensionFreshEvalKey {
    fn eq(&self, other: &Self) -> bool {
        self.extension_id == other.extension_id
            && self.input_hash == other.input_hash
            && self.bzl_transitive_digest == other.bzl_transitive_digest
            && self.root_module_name == other.root_module_name
            && self.project_root == other.project_root
            && self.workspace_id == other.workspace_id
            && facts_identity(&self.prior_facts) == facts_identity(&other.prior_facts)
            && self.repo_env == other.repo_env
            && self.repo_mappings == other.repo_mappings
            && self.repo_mapping_overrides == other.repo_mapping_overrides
    }
}

impl Eq for ModuleExtensionFreshEvalKey {}

#[async_trait]
impl Key for ModuleExtensionFreshEvalKey {
    type Value = slug_error::Result<Arc<ModuleExtensionResult>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let module_count = self.aggregated.tags_by_module.len();
        let tag_count: usize = self
            .aggregated
            .tags_by_module
            .values()
            .map(|v| v.len())
            .sum();
        tracing::debug!(
            "Extension '{}' used by {} module(s) with {} total tag(s)",
            self.extension_id,
            module_count,
            tag_count
        );

        record_bzlmod_event(BzlmodEventKind::ExtensionEval, self.extension_id.as_ref());
        let temp_dir = create_temp_extension_dir(&self.extension_id)?;

        let execution_result = match MODULE_EXTENSION_EXECUTOR_IMPL.get() {
            Ok(executor) => {
                executor
                    .execute_extension(
                        ctx,
                        &self.aggregated,
                        &self.root_module_name,
                        &temp_dir,
                        self.prior_facts.as_ref().clone(),
                        self.repo_env.clone(),
                        self.bzl_transitive_digest.clone(),
                        self.workspace_id.clone(),
                    )
                    .await
            }
            Err(e) => Err(ModuleExtensionError::ExecutionFailed {
                extension_id: self.extension_id.to_string(),
                reason: format!("module extension executor is not initialized: {e}"),
            }
            .into()),
        };

        if temp_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&temp_dir) {
                tracing::warn!(
                    "Failed to clean up temp dir for extension '{}': {}",
                    self.extension_id,
                    e
                );
            }
        }

        let mut output = execution_result?;
        output.recorded_inputs = validate_fresh_recorded_inputs_dependency(
            ctx,
            &self.extension_id,
            output.recorded_inputs,
            self.workspace_id.clone(),
            self.repo_env.clone(),
            self.repo_mappings.clone(),
        )
        .await?;
        tracing::debug!(
            "Extension '{}' recorded {} input(s)",
            self.extension_id,
            output.recorded_inputs.len()
        );

        let result = ModuleExtensionResult::new_with_metadata_and_recorded_input_context(
            self.extension_id.clone(),
            self.input_hash.to_string(),
            output.generated_repo_specs.clone(),
            &self.root_module_name,
            output.metadata.clone(),
            output.recorded_inputs.clone(),
            ModuleExtensionRecordedInputContext::new(
                self.project_root.as_deref().cloned(),
                self.repo_env.as_ref().clone(),
                self.repo_mappings.as_ref().clone(),
            ),
        );

        tracing::info!(
            "Extension '{}' generated {} repository specs",
            self.extension_id,
            result.repo_count()
        );

        Ok(Arc::new(result))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
}

#[async_trait]
impl Key for ModuleExtensionExecutionKey {
    type Value = slug_error::Result<Arc<ModuleExtensionResult>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        tracing::info!(
            "Evaluating module extension '{}' (input_hash: {})",
            self.extension_id,
            self.input_hash
        );

        let prior_facts = self.replay_inputs.prior_facts.clone();
        let workspace_lockfile_facts = self.replay_inputs.workspace_lockfile_facts.clone();

        // 1. Consume replay inputs selected by `ModuleExtensionReplayInputsKey`.
        //    Extension execution must not reopen lockfiles or decide which
        //    visible/hidden entry applies; it only validates recorded inputs
        //    before accepting a selected cache hit.
        if let Some(selected_cache) = self.replay_inputs.selected_cache.clone()
            && selected_cache_recorded_inputs_current(
                ctx,
                self.workspace_id.clone(),
                &self.extension_id,
                &selected_cache,
            )
            .await?
        {
            let source = self
                .replay_inputs
                .selected_cache_identity
                .as_ref()
                .map(|identity| identity.source);
            match source {
                Some(crate::dice_graph::LockfileContentKind::Hidden) => tracing::info!(
                    "Extension '{}' hidden lockfile cache HIT: using {} cached repo specs",
                    self.extension_id,
                    selected_cache.repo_specs.len()
                ),
                _ => tracing::info!(
                    "Extension '{}' cache HIT: using {} cached repo specs",
                    self.extension_id,
                    selected_cache.repo_specs.len()
                ),
            }
            selected_cache.record_hit(&self.extension_id);

            let recorded_input_context =
                ModuleExtensionRecordedInputContext::from_selected_cache(&selected_cache);
            let result = ModuleExtensionResult::new_with_metadata_and_recorded_input_context(
                self.extension_id.clone(),
                self.input_hash.to_string(),
                selected_cache.repo_specs,
                &self.root_module_name,
                ModuleExtensionMetadata {
                    facts: prior_facts.clone(),
                },
                selected_cache.recorded_inputs.clone(),
                recorded_input_context,
            );

            return Ok(Arc::new(result));
        }

        let result = ctx
            .compute(&ModuleExtensionFreshEvalKey::from_execution_key(
                self,
                Arc::new(prior_facts),
            ))
            .await??;
        validate_error_mode_facts(
            &self.extension_id,
            self.replay_inputs.lockfile_mode,
            &result.metadata.facts,
            &workspace_lockfile_facts,
        )?;

        Ok(result)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        // Don't cache errors - retry on next request
        x.is_ok()
    }
}

/// Create a temporary working directory for extension execution.
///
/// The directory is created under the system temp directory with a name
/// derived from the extension ID. This directory is for `module_ctx` I/O
/// and is deleted after the extension completes.
fn create_temp_extension_dir(extension_id: &str) -> slug_error::Result<PathBuf> {
    // Sanitize extension ID for use in path
    let sanitized = sanitize_extension_id_for_path(extension_id);

    let temp_base = std::env::temp_dir().join("slug-extension");
    std::fs::create_dir_all(&temp_base).map_err(|e| ModuleExtensionError::TempDirFailed {
        extension_id: extension_id.to_owned(),
        reason: format!("failed to create temp base: {}", e),
    })?;

    let temp_dir = temp_base.join(&sanitized);

    // Clean up any previous temp dir for this extension
    if temp_dir.exists() {
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    std::fs::create_dir_all(&temp_dir).map_err(|e| ModuleExtensionError::TempDirFailed {
        extension_id: extension_id.to_owned(),
        reason: format!("failed to create temp dir: {}", e),
    })?;

    Ok(temp_dir)
}

/// Sanitize an extension ID for use in a filesystem path.
///
/// Replaces characters that are problematic in paths with underscores.
fn sanitize_extension_id_for_path(extension_id: &str) -> String {
    extension_id
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '@' | '%' | ' ' => '_',
            c if c.is_alphanumeric() || c == '_' || c == '-' || c == '+' || c == '.' => c,
            _ => '_',
        })
        .collect()
}

/// Build canonical names for extension-generated repositories.
///
/// Format: `{owning_module}+{extension_unique_name}+{internal_name}`
/// where `owning_module` is `_main` for extensions defined in the root module
/// (Bazel 9 convention; see `extract_owning_module`) and the canonical module
/// repo name otherwise. The separator is `+`, so non-root module extensions can
/// have Bazel's `rules_rs++crate+repo` shape.
///
/// `root_module_name` is the value of `module(name=…)` in the root MODULE.bazel
/// — required so the root module's declared name (e.g., `llvm-project-overlay`)
/// is canonicalized to `_main`, matching what `pending_repo_cells.rs` registers
/// when it pre-computes the same cells from `use_repo()` declarations.
pub fn build_canonical_names(
    extension_id: &str,
    specs: &FxHashMap<String, RepoSpec>,
    root_module_name: &str,
) -> FxHashMap<String, String> {
    let ext_name = extract_extension_name(extension_id);
    let owning_module = extract_owning_module(extension_id, root_module_name);
    specs
        .keys()
        .map(|internal| {
            let canonical = format!("{}+{}+{}", owning_module, ext_name, internal);
            (internal.clone(), canonical)
        })
        .collect()
}

/// Extract the canonical owning-module prefix from an extension ID.
///
/// Bazel's canonical naming convention prefixes the *root* module's extension
/// repos with the literal string `_main`, regardless of the name the root
/// module declares in MODULE.bazel. Non-root modules use their canonical module
/// repo name, including Bazel 9's trailing `+`.
/// `root_module_name` is the value of `module(name=…)` in the root MODULE.bazel
/// (e.g., `llvm-project-overlay`); pass `""` if the build has no root module.
///
/// Without this substitution, the root module's own extension defined at
/// `@<root_module>//ext.bzl` would be canonicalized as
/// `<root_module>+ext+repo` while `pending_repo_cells.rs` (using the
/// `_main`-rule) registers it as `_main+ext+repo`. The two paths point to
/// different `bazel-external/...` directories and the build fails with
/// "package not found" once the repo rule tries to read its own files.
///
/// Extension ID formats:
/// - `@bazel_features//private:extensions.bzl%version_extension` → `bazel_features+`
/// - `@@rules_cc+//cc:extensions.bzl%cc_configure` → `rules_cc+`
/// - `//path:file.bzl%ext` → `_main` (root module, no repo prefix)
/// - `@<root_module_name>//path:file.bzl%ext` → `_main` (root module via its declared name)
///
/// Falls back to `_main` if the format doesn't match.
pub fn extract_owning_module(extension_id: &str, root_module_name: &str) -> String {
    // Strip the extension name part (after %)
    let bzl_part = extension_id.split('%').next().unwrap_or(extension_id);

    // Look for @module// or @@module// pattern.
    //
    // The module segment may appear in two shapes:
    //   - slug internal:        `@<apparent>//...`        → module = "<apparent>"
    //   - bazel 9 canonical:    `@@<repo>+//...`          → module = "<repo>+"
    // Bazel 9 includes the trailing `+` in canonical module repository names,
    // and extension-generated repositories preserve it before adding their own
    // `+<extension>+<repo>` suffix.
    let stripped = bzl_part
        .strip_prefix("@@")
        .or_else(|| bzl_part.strip_prefix('@'))
        .unwrap_or(bzl_part);
    if let Some(pos) = stripped.find("//") {
        let module = &stripped[..pos];
        if !module.is_empty() {
            // Map the root module's declared name back to Bazel's canonical
            // `_main` placeholder so callers all agree on one canonical
            // prefix per repo, no matter which spelling of the root module
            // they observe in extension IDs / Starlark labels.
            if !root_module_name.is_empty() && module == root_module_name {
                return "_main".to_owned();
            }
            if module.ends_with('+') {
                return module.to_owned();
            }
            return format!("{module}+");
        }
    }

    // No module prefix (e.g., "//path:file.bzl") means root module
    "_main".to_owned()
}

/// Extract the extension name from an extension ID.
///
/// Extension ID format: `@@module//path:file.bzl%extension_name`
/// Returns the `extension_name` part.
///
/// If the format doesn't match, returns the entire ID (sanitized).
pub fn extract_extension_name(extension_id: &str) -> String {
    // Look for %extension_name at the end
    if let Some(pos) = extension_id.rfind('%') {
        extension_id[pos + 1..].to_owned()
    } else if let Some(pos) = extension_id.rfind(':') {
        // Fallback: try to use the bzl file name without extension
        let after_colon = &extension_id[pos + 1..];
        after_colon
            .strip_suffix(".bzl")
            .unwrap_or(after_colon)
            .to_owned()
    } else {
        // Last resort: use the whole thing, sanitized
        extension_id
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect()
    }
}

/// Compute a transitive digest for the extension's .bzl files.
///
/// Ideally, this would hash all .bzl files that the extension transitively depends on.
/// For now, we use a simplified approach that hashes the extension ID. This provides
/// basic cache invalidation when the extension changes but doesn't capture all
/// transitive .bzl file changes.
///
/// TODO: Improve this by integrating with the Starlark module loading system
/// to get the actual transitive digest of all loaded .bzl files.
pub fn compute_bzl_transitive_digest(extension_id: &str) -> String {
    use base64::Engine;
    use sha2::Digest;
    use sha2::Sha256;

    let mut hasher = Sha256::new();
    hasher.update(b"bzl_transitive_v1:");
    hasher.update(extension_id.as_bytes());

    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}

pub fn compute_bzl_transitive_digest_from_file_states(
    extension_id: &str,
    file_states: &BTreeMap<String, Result<String, String>>,
) -> String {
    if file_states.is_empty() {
        return compute_bzl_transitive_digest(extension_id);
    }

    use base64::Engine;
    use sha2::Digest;
    use sha2::Sha256;

    let mut hasher = Sha256::new();
    hasher.update(b"bzl_transitive_v2:");
    hasher.update(extension_id.as_bytes());
    hasher.update([0]);
    for (path, state) in file_states {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        match state {
            Ok(content) => hasher.update(content.as_bytes()),
            Err(error) => {
                hasher.update(b"read_error:");
                hasher.update(error.as_bytes());
            }
        }
        hasher.update([0]);
    }

    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}

/// Compute the fallback best-effort Bazel-shaped transitive digest for
/// workspace-local extension `.bzl` files.
///
/// Bazel computes this from the loaded module graph. Slug does not yet expose
/// This is not the normal DICE extension replay digest. Bazel computes that
/// digest from the loaded module graph; Slug's DICE path uses the loaded graph
/// through `ExtensionBzlTransitiveDigestKey`. This fallback remains only for
/// bootstrap/preseed callers that run before current extension aggregation is
/// injected. It hashes files that can be resolved under `project_root` from
/// literal `load()` statements. If the extension file cannot be resolved
/// locally, it falls back to the old extension-id digest so external/registry
/// cases keep their existing behavior until the remaining bridge is removed.
#[cfg(test)]
fn compute_bzl_transitive_digest_for_project(extension_id: &str, project_root: &Path) -> String {
    compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
        extension_id,
        project_root,
        None,
    )
}

#[cfg(test)]
fn compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
    extension_id: &str,
    project_root: &Path,
    repo_mappings: Option<&RepoMappingSnapshot>,
) -> String {
    let Some(root_bzl) =
        extension_bzl_location_under_project(extension_id, project_root, repo_mappings)
    else {
        return compute_bzl_transitive_digest(extension_id);
    };
    if !root_bzl.path.is_file() {
        return compute_bzl_transitive_digest(extension_id);
    }

    let mut seen_locations = BTreeSet::new();
    let mut seen_files = BTreeSet::new();
    collect_bzl_transitive_files(
        project_root,
        root_bzl,
        repo_mappings,
        &mut seen_locations,
        &mut seen_files,
    );
    if seen_files.is_empty() {
        return compute_bzl_transitive_digest(extension_id);
    }

    use base64::Engine;
    use sha2::Digest;
    use sha2::Sha256;

    let mut hasher = Sha256::new();
    hasher.update(b"bzl_transitive_v2:");
    hasher.update(extension_id.as_bytes());
    hasher.update([0]);
    for path in seen_files {
        let rel = path.strip_prefix(project_root).unwrap_or(path.as_path());
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update([0]);
        match std::fs::read(&path) {
            Ok(content) => hasher.update(content),
            Err(e) => {
                hasher.update(b"read_error:");
                hasher.update(e.to_string().as_bytes());
            }
        }
        hasher.update([0]);
    }

    let hash = hasher.finalize();
    base64::engine::general_purpose::STANDARD.encode(hash)
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct BzlLoadLocation {
    pub path: PathBuf,
    pub repo: String,
    pub package: String,
}

#[cfg(test)]
fn extension_bzl_location_under_project(
    extension_id: &str,
    project_root: &Path,
    repo_mappings: Option<&RepoMappingSnapshot>,
) -> Option<BzlLoadLocation> {
    let label = extension_id.split('%').next().unwrap_or(extension_id);
    label_bzl_location_under_project(label, project_root, None, repo_mappings)
}

#[cfg(test)]
fn label_bzl_location_under_project(
    label: &str,
    project_root: &Path,
    current: Option<&BzlLoadLocation>,
    repo_mappings: Option<&RepoMappingSnapshot>,
) -> Option<BzlLoadLocation> {
    if let Some(rest) = label.strip_prefix("@@") {
        let (repo, target) = rest.split_once("//")?;
        let (pkg, name) = split_bzl_label_target(target)?;
        if let Some(location) = bzl_location_for_repo(repo, pkg, name, project_root) {
            return Some(location);
        }
        if repo.contains('+') {
            return missing_bzl_location_for_repo(repo, pkg, name, project_root);
        }
        if !repo.contains('+') {
            return Some(project_bzl_location(project_root, pkg, name));
        }
        None
    } else if let Some(rest) = label.strip_prefix('@') {
        let (repo, target) = rest.split_once("//")?;
        let (pkg, name) = split_bzl_label_target(target)?;
        let mut mapped = false;
        let repo = if repo.contains('+') {
            repo.to_owned()
        } else if let (Some(current), Some(repo_mappings)) = (current, repo_mappings) {
            if let Some(canonical_repo) =
                mapped_repo_for_apparent(repo_mappings, &current.repo, repo)
            {
                mapped = canonical_repo != repo;
                canonical_repo
            } else {
                repo.to_owned()
            }
        } else {
            repo.to_owned()
        };
        if let Some(location) = bzl_location_for_repo(&repo, pkg, name, project_root) {
            return Some(location);
        }
        if mapped || repo.contains('+') {
            return missing_bzl_location_for_repo(&repo, pkg, name, project_root);
        }
        if !mapped && !repo.contains('+') {
            return Some(project_bzl_location(project_root, pkg, name));
        }
        None
    } else if let Some(rest) = label.strip_prefix("//") {
        let current_repo = current.map(|location| location.repo.as_str()).unwrap_or("");
        let (pkg, name) = split_bzl_label_target(rest)?;
        bzl_location_for_repo(current_repo, pkg, name, project_root).or_else(|| {
            (!current_repo.is_empty() && current_repo != "_main")
                .then(|| missing_bzl_location_for_repo(current_repo, pkg, name, project_root))
                .flatten()
        })
    } else if let Some(name) = label.strip_prefix(':') {
        let current = current?;
        bzl_location_for_repo(&current.repo, &current.package, name, project_root).or_else(|| {
            (!current.repo.is_empty() && current.repo != "_main")
                .then(|| {
                    missing_bzl_location_for_repo(
                        &current.repo,
                        &current.package,
                        name,
                        project_root,
                    )
                })
                .flatten()
        })
    } else if label.contains("//") {
        let (repo, target) = label.split_once("//")?;
        let (pkg, name) = split_bzl_label_target(target)?;
        let mut mapped = false;
        let repo = if repo.contains('+') {
            repo.to_owned()
        } else if let (Some(current), Some(repo_mappings)) = (current, repo_mappings) {
            if let Some(canonical_repo) =
                mapped_repo_for_apparent(repo_mappings, &current.repo, repo)
            {
                mapped = canonical_repo != repo;
                canonical_repo
            } else {
                repo.to_owned()
            }
        } else {
            repo.to_owned()
        };
        if let Some(location) = bzl_location_for_repo(&repo, pkg, name, project_root) {
            return Some(location);
        }
        if mapped || repo.contains('+') {
            return missing_bzl_location_for_repo(&repo, pkg, name, project_root);
        }
        if !mapped && !repo.contains('+') {
            return Some(project_bzl_location(project_root, pkg, name));
        }
        None
    } else {
        let current = current?;
        let mut path = current.path.parent()?.to_path_buf();
        path.push(label);
        Some(BzlLoadLocation {
            path,
            repo: current.repo.clone(),
            package: current.package.clone(),
        })
    }
}

#[cfg(test)]
fn bzl_location_for_repo(
    repo: &str,
    package: &str,
    name: &str,
    project_root: &Path,
) -> Option<BzlLoadLocation> {
    bzl_location_for_repo_impl(repo, package, name, project_root, false)
}

#[cfg(test)]
fn missing_bzl_location_for_repo(
    repo: &str,
    package: &str,
    name: &str,
    project_root: &Path,
) -> Option<BzlLoadLocation> {
    bzl_location_for_repo_impl(repo, package, name, project_root, true)
}

#[cfg(test)]
fn bzl_location_for_repo_impl(
    repo: &str,
    package: &str,
    name: &str,
    project_root: &Path,
    include_missing: bool,
) -> Option<BzlLoadLocation> {
    if repo.is_empty() || repo == "_main" {
        return Some(project_bzl_location(project_root, package, name));
    }

    let mut first_missing = None;
    for candidate in external_repo_candidates(repo) {
        let mut path = project_root.join("bazel-external").join(&candidate);
        if !package.is_empty() {
            path.push(package);
        }
        path.push(name);
        if path.is_file() {
            return Some(BzlLoadLocation {
                path,
                repo: candidate,
                package: package.to_owned(),
            });
        }
        first_missing.get_or_insert_with(|| BzlLoadLocation {
            path,
            repo: candidate,
            package: package.to_owned(),
        });
    }
    include_missing.then_some(first_missing).flatten()
}

#[cfg(test)]
fn project_bzl_location(project_root: &Path, package: &str, name: &str) -> BzlLoadLocation {
    let mut path = project_root.to_path_buf();
    if !package.is_empty() {
        path.push(package);
    }
    path.push(name);
    BzlLoadLocation {
        path,
        repo: String::new(),
        package: package.to_owned(),
    }
}

#[cfg(test)]
fn external_repo_candidates(repo: &str) -> Vec<String> {
    if repo.is_empty() || repo == "_main" {
        return Vec::new();
    }
    let mut candidates = vec![repo.to_owned()];
    if repo.ends_with('+') {
        candidates.push(repo.trim_end_matches('+').to_owned());
    } else if !repo.contains('+') {
        candidates.push(format!("{repo}+"));
    }
    candidates
}

#[cfg(test)]
fn mapped_repo_for_apparent(
    repo_mappings: &RepoMappingSnapshot,
    current_repo: &str,
    apparent_repo: &str,
) -> Option<String> {
    mapping_for_source_repo(repo_mappings, current_repo)
        .and_then(|mapping| mapping.get(apparent_repo).cloned())
}

#[cfg(test)]
fn mapping_for_source_repo<'a>(
    repo_mappings: &'a RepoMappingSnapshot,
    current_repo: &str,
) -> Option<&'a BTreeMap<String, String>> {
    for candidate in source_repo_mapping_candidates(current_repo) {
        if let Some(mapping) = repo_mappings.get(&candidate) {
            return Some(mapping);
        }
    }
    None
}

#[cfg(test)]
fn source_repo_mapping_candidates(current_repo: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    push_unique_candidate(&mut candidates, current_repo.to_owned());
    if current_repo.is_empty() || current_repo == "_main" {
        push_unique_candidate(&mut candidates, "_main".to_owned());
        push_unique_candidate(&mut candidates, String::new());
    } else if current_repo.ends_with('+') {
        push_unique_candidate(
            &mut candidates,
            current_repo.trim_end_matches('+').to_owned(),
        );
    } else if !current_repo.contains('+') {
        push_unique_candidate(&mut candidates, format!("{current_repo}+"));
    }
    candidates
}

#[cfg(test)]
fn push_unique_candidate(candidates: &mut Vec<String>, candidate: String) {
    if !candidates.contains(&candidate) {
        candidates.push(candidate);
    }
}

#[cfg(test)]
fn split_bzl_label_target(target: &str) -> Option<(&str, &str)> {
    target.split_once(':')
}

#[cfg(test)]
fn collect_bzl_transitive_files(
    project_root: &Path,
    location: BzlLoadLocation,
    repo_mappings: Option<&RepoMappingSnapshot>,
    seen_locations: &mut BTreeSet<(PathBuf, String, String)>,
    seen_files: &mut BTreeSet<PathBuf>,
) {
    let key = (
        location.path.clone(),
        location.repo.clone(),
        location.package.clone(),
    );
    if !seen_locations.insert(key) {
        return;
    }
    seen_files.insert(location.path.clone());
    let path = location.path.clone();
    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };
    for load in literal_loads(&path, &content) {
        let Some(load_location) =
            label_bzl_location_under_project(&load, project_root, Some(&location), repo_mappings)
        else {
            continue;
        };
        if load_location.path.starts_with(project_root) {
            collect_bzl_transitive_files(
                project_root,
                load_location,
                repo_mappings,
                seen_locations,
                seen_files,
            );
        }
    }
}

#[cfg(test)]
fn literal_loads(path: &Path, content: &str) -> Vec<String> {
    let filename = path.to_string_lossy().into_owned();
    let Ok(ast) = AstModule::parse(&filename, content.to_owned(), &Dialect::Standard) else {
        return Vec::new();
    };
    let mut loads = Vec::new();
    collect_literal_loads_from_stmt(ast.statement(), &mut loads);
    loads
}

#[cfg(test)]
fn collect_literal_loads_from_stmt(stmt: &AstStmt, loads: &mut Vec<String>) {
    match &stmt.node {
        StmtP::Statements(stmts) => {
            for stmt in stmts {
                collect_literal_loads_from_stmt(stmt, loads);
            }
        }
        StmtP::Load(load) => loads.push(load.module.node.clone()),
        StmtP::If(_, body) => collect_literal_loads_from_stmt(body, loads),
        StmtP::IfElse(_, branches) => {
            collect_literal_loads_from_stmt(&branches.0, loads);
            collect_literal_loads_from_stmt(&branches.1, loads);
        }
        StmtP::For(for_stmt) => collect_literal_loads_from_stmt(&for_stmt.body, loads),
        StmtP::Def(def) => collect_literal_loads_from_stmt(&def.body, loads),
        StmtP::Break
        | StmtP::Continue
        | StmtP::Pass
        | StmtP::Return(_)
        | StmtP::Expression(_)
        | StmtP::Assign(_)
        | StmtP::AssignModify(_, _, _) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository_invocations::AttrValue;

    #[test]
    fn test_module_extension_result_creation() {
        let mut specs = FxHashMap::default();
        specs.insert(
            "numpy".to_owned(),
            RepoSpec::new("@@rules_python//...%pip_install".to_owned())
                .with_attr("version".to_owned(), AttrValue::String("1.24.0".to_owned())),
        );
        specs.insert(
            "requests".to_owned(),
            RepoSpec::new("@@rules_python//...%pip_install".to_owned())
                .with_attr("version".to_owned(), AttrValue::String("2.31.0".to_owned())),
        );

        let result = ModuleExtensionResult::new(
            Arc::from("@@rules_python//python/pip:pip.bzl%pip"),
            "sha256-abc123".to_owned(),
            specs,
            "",
        );

        assert_eq!(
            result.extension_id.as_ref(),
            "@@rules_python//python/pip:pip.bzl%pip"
        );
        assert_eq!(result.input_hash, "sha256-abc123");
        assert_eq!(result.repo_count(), 2);
        assert!(result.contains_repo("numpy"));
        assert!(result.contains_repo("requests"));
        assert!(!result.contains_repo("pandas"));
    }

    #[test]
    fn test_canonical_name_lookup() {
        let mut specs = FxHashMap::default();
        specs.insert("foo".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("bar".to_owned(), RepoSpec::new("rule".to_owned()));

        let result = ModuleExtensionResult::new(
            Arc::from("@@module//path:ext.bzl%my_extension"),
            "hash".to_owned(),
            specs,
            "",
        );

        assert_eq!(
            result.canonical_name("foo"),
            Some("module++my_extension+foo")
        );
        assert_eq!(
            result.canonical_name("bar"),
            Some("module++my_extension+bar")
        );
        assert_eq!(result.canonical_name("baz"), None);
    }

    #[test]
    fn test_internal_name_from_canonical() {
        let mut specs = FxHashMap::default();
        specs.insert("numpy".to_owned(), RepoSpec::new("rule".to_owned()));

        let result = ModuleExtensionResult::new(
            Arc::from("@@rules_python//pip:pip.bzl%pip"),
            "hash".to_owned(),
            specs,
            "",
        );

        assert_eq!(
            result.internal_name_from_canonical("rules_python++pip+numpy"),
            Some("numpy")
        );
        assert_eq!(
            result.internal_name_from_canonical("rules_python++pip+pandas"),
            None
        );
    }

    #[test]
    fn test_extract_extension_name() {
        assert_eq!(
            extract_extension_name("@@rules_python//pip:pip.bzl%pip"),
            "pip"
        );
        assert_eq!(
            extract_extension_name("@@bazel_features//private:extensions.bzl%bazel_features"),
            "bazel_features"
        );
        assert_eq!(
            extract_extension_name("//:my_extension.bzl%my_ext"),
            "my_ext"
        );
        // Fallback cases
        assert_eq!(extract_extension_name("//:extension.bzl"), "extension");
        assert_eq!(extract_extension_name("simple_name"), "simple_name");
    }

    #[test]
    fn test_build_canonical_names() {
        let mut specs = FxHashMap::default();
        specs.insert("numpy".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("pandas".to_owned(), RepoSpec::new("rule".to_owned()));

        let names = build_canonical_names("@@rules_python//pip:pip.bzl%pip", &specs, "");

        assert_eq!(
            names.get("numpy"),
            Some(&"rules_python++pip+numpy".to_owned())
        );
        assert_eq!(
            names.get("pandas"),
            Some(&"rules_python++pip+pandas".to_owned())
        );
    }

    #[test]
    fn extension_id_for_canonical_repo_matches_owner_module() {
        use crate::BzlmodExtensionAggregationsDataValue;
        use crate::extensions::AggregatedExtension;

        let workspace_id =
            crate::WorkspaceId::for_project_root(PathBuf::from("/tmp/slug-plan61-ext-data"));
        let mut extensions = std::collections::HashMap::new();
        let mut root_ext = AggregatedExtension::new("@root//:ext.bzl", "ext");
        root_ext.extension_id = "@root//:ext.bzl%ext".to_owned();
        let mut dep_ext = AggregatedExtension::new("@dep//:ext.bzl", "ext");
        dep_ext.extension_id = "@dep//:ext.bzl%ext".to_owned();
        extensions.insert(root_ext.extension_id.clone(), root_ext);
        extensions.insert(dep_ext.extension_id.clone(), dep_ext);
        let data = BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
            workspace_id,
            "root".to_owned(),
            Arc::new(extensions),
        );

        let dep_id =
            extension_id_for_canonical_repo(&data, &data.root_module_name, "dep++ext+tool")
                .unwrap();
        assert_eq!(dep_id, "@dep//:ext.bzl%ext");

        let root_id =
            extension_id_for_canonical_repo(&data, &data.root_module_name, "_main+ext+tool")
                .unwrap();
        assert_eq!(root_id, "@root//:ext.bzl%ext");
    }

    #[test]
    fn extension_spokes_lookup_keys_recompute_replay_inputs() {
        let missing_extension: slug_error::Result<Option<Arc<ExtensionSpokesValue>>> = Ok(None);
        let failed_lookup: slug_error::Result<Option<Arc<ExtensionSpokesValue>>> = Err(
            slug_error::slug_error!(slug_error::ErrorTag::Tier0, "lookup failed"),
        );
        let missing_aggregation: slug_error::Result<Option<Arc<BzlmodExtensionAggregationValue>>> =
            Ok(None);
        let failed_aggregation: slug_error::Result<Option<Arc<BzlmodExtensionAggregationValue>>> =
            Err(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "aggregation failed"
            ));
        let missing_canonical_owner: slug_error::Result<Option<Arc<str>>> = Ok(None);
        let failed_canonical_owner: slug_error::Result<Option<Arc<str>>> = Err(
            slug_error::slug_error!(slug_error::ErrorTag::Tier0, "canonical owner failed"),
        );
        let first_digest: slug_error::Result<Arc<ExtensionBzlTransitiveDigestValue>> = Ok(
            Arc::new(ExtensionBzlTransitiveDigestValue::new("first".to_owned())),
        );
        let same_digest: slug_error::Result<Arc<ExtensionBzlTransitiveDigestValue>> = Ok(Arc::new(
            ExtensionBzlTransitiveDigestValue::new("first".to_owned()),
        ));
        let changed_digest: slug_error::Result<Arc<ExtensionBzlTransitiveDigestValue>> = Ok(
            Arc::new(ExtensionBzlTransitiveDigestValue::new("second".to_owned())),
        );
        let failed_digest: slug_error::Result<Arc<ExtensionBzlTransitiveDigestValue>> = Err(
            slug_error::slug_error!(slug_error::ErrorTag::Tier0, "digest failed"),
        );

        assert!(!<BzlmodExtensionAggregationKey as Key>::validity(
            &missing_aggregation
        ));
        assert!(!<BzlmodExtensionAggregationKey as Key>::validity(
            &failed_aggregation
        ));
        assert!(<ExtensionIdByCanonicalRepoKey as Key>::validity(
            &missing_canonical_owner
        ));
        assert!(!<ExtensionIdByCanonicalRepoKey as Key>::validity(
            &failed_canonical_owner
        ));
        assert!(!<ExtensionSpokesByExtensionIdKey as Key>::validity(
            &missing_extension
        ));
        assert!(!<ExtensionSpokesByCanonicalRepoKey as Key>::validity(
            &missing_extension
        ));
        assert!(!<ExtensionSpokesByExtensionIdKey as Key>::validity(
            &failed_lookup
        ));
        assert!(!<ExtensionSpokesByCanonicalRepoKey as Key>::validity(
            &failed_lookup
        ));

        assert!(<ExtensionBzlTransitiveDigestKey as Key>::equality(
            &first_digest,
            &same_digest
        ));
        assert!(!<ExtensionBzlTransitiveDigestKey as Key>::equality(
            &first_digest,
            &changed_digest
        ));
        assert!(!<ExtensionBzlTransitiveDigestKey as Key>::validity(
            &first_digest
        ));
        assert!(!<ExtensionBzlTransitiveDigestKey as Key>::validity(
            &failed_digest
        ));
    }

    #[tokio::test]
    async fn missing_extension_spoke_lookup_does_not_require_replay_inputs()
    -> slug_error::Result<()> {
        let project_root = PathBuf::from("/tmp/slug-plan61-missing-spoke-lookup");
        let workspace_id = crate::WorkspaceId::for_project_root(project_root.clone());
        let aggregations = Arc::new(
            BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                workspace_id.clone(),
                String::new(),
                Arc::new(std::collections::HashMap::new()),
            ),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(BzlmodExtensionAggregationsDataKey, aggregations)])?;
        let mut dice = updater.commit().await;

        let by_id = ExtensionSpokesByExtensionIdKey::for_project_root(
            project_root,
            "@root//:missing.bzl%missing",
        );
        assert!(dice.compute(&by_id).await??.is_none());

        let by_canonical =
            ExtensionSpokesByCanonicalRepoKey::for_workspace_id(workspace_id, "_main+missing+repo");
        assert!(dice.compute(&by_canonical).await??.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn extension_bzl_digest_key_rejects_missing_aggregation() -> slug_error::Result<()> {
        let project_root = PathBuf::from("/tmp/slug-plan61-extension-digest-missing-aggregation");
        let workspace_id = crate::WorkspaceId::for_project_root(project_root);
        let extension_id = "@root//:missing.bzl%missing";
        let aggregations = Arc::new(
            BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                workspace_id.clone(),
                String::new(),
                Arc::new(std::collections::HashMap::new()),
            ),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(BzlmodExtensionAggregationsDataKey, aggregations)])?;
        let mut dice = updater.commit().await;
        let key = ExtensionBzlTransitiveDigestKey {
            workspace_id,
            extension_id: Arc::from(extension_id),
            allow_missing_loads: false,
        };
        let err = dice.compute(&key).await?.unwrap_err();

        assert!(
            err.to_string()
                .contains("not found while computing loaded .bzl digest"),
            "{err:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn extension_aggregation_key_projects_single_extension() -> slug_error::Result<()> {
        fn aggregations_value(
            workspace_id: crate::WorkspaceId,
            target: AggregatedExtension,
            other: AggregatedExtension,
        ) -> Arc<BzlmodExtensionAggregationsDataValue> {
            let mut extension_aggregations = std::collections::HashMap::new();
            extension_aggregations.insert(target.extension_id.clone(), target);
            extension_aggregations.insert(other.extension_id.clone(), other);
            Arc::new(
                BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                    workspace_id,
                    "root".to_owned(),
                    Arc::new(extension_aggregations),
                ),
            )
        }

        let workspace_id =
            crate::WorkspaceId::for_project_root(PathBuf::from("/tmp/slug-plan61-aggregation-key"));
        let mut target = AggregatedExtension::new("@root//:ext.bzl", "ext");
        target.extension_id = "@root//:ext.bzl%ext".to_owned();
        target.add_imported_repos(["target_repo".to_owned()]);
        let mut other = AggregatedExtension::new("@other//:ext.bzl", "ext");
        other.extension_id = "@other//:ext.bzl%ext".to_owned();
        other.add_imported_repos(["other_repo".to_owned()]);
        let key = BzlmodExtensionAggregationKey::for_workspace_id(
            workspace_id.clone(),
            &target.extension_id,
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            aggregations_value(workspace_id.clone(), target.clone(), other.clone()),
        )])?;
        let mut dice = updater.commit().await;
        let first = dice.compute(&key).await??.unwrap();
        assert_eq!(first.root_module_name.as_ref(), "root");

        let mut changed_other = other.clone();
        changed_other.add_imported_repos(["changed_other_repo".to_owned()]);
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            aggregations_value(workspace_id.clone(), target.clone(), changed_other),
        )])?;
        let mut dice = updater.commit().await;
        let second = dice.compute(&key).await??.unwrap();
        assert_eq!(first, second);

        let mut changed_target = target.clone();
        changed_target.add_imported_repos(["changed_target_repo".to_owned()]);
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            aggregations_value(workspace_id.clone(), changed_target, other),
        )])?;
        let mut dice = updater.commit().await;
        let third = dice.compute(&key).await??.unwrap();
        assert_ne!(first, third);

        Ok(())
    }

    #[tokio::test]
    async fn extension_aggregation_data_rejects_wrong_workspace() -> slug_error::Result<()> {
        let workspace_id = crate::WorkspaceId::for_project_root(PathBuf::from(
            "/tmp/slug-plan61-aggregation-key-workspace-a",
        ));
        let other_workspace_id = crate::WorkspaceId::for_project_root(PathBuf::from(
            "/tmp/slug-plan61-aggregation-key-workspace-b",
        ));
        let mut target = AggregatedExtension::new("@root//:ext.bzl", "ext");
        target.extension_id = "@root//:ext.bzl%ext".to_owned();
        let extension_id = target.extension_id.clone();
        let mut extension_aggregations = std::collections::HashMap::new();
        extension_aggregations.insert(extension_id.clone(), target);
        let key =
            BzlmodExtensionAggregationKey::for_workspace_id(workspace_id.clone(), &extension_id);

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            Arc::new(
                BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                    other_workspace_id,
                    "root".to_owned(),
                    Arc::new(extension_aggregations),
                ),
            ),
        )])?;
        let mut dice = updater.commit().await;
        let err = dice.compute(&key).await?.unwrap_err();

        assert!(err.to_string().contains("aggregation data root"), "{err:?}");
        Ok(())
    }

    #[tokio::test]
    async fn extension_bzl_digest_key_requires_executor_when_aggregation_exists()
    -> slug_error::Result<()> {
        let workspace_id = crate::WorkspaceId::for_project_root(PathBuf::from(
            "/tmp/slug-plan61-extension-digest-requires-executor",
        ));
        let mut target = AggregatedExtension::new("@root//:ext.bzl", "ext");
        target.extension_id = "@root//:ext.bzl%ext".to_owned();
        let extension_id = target.extension_id.clone();
        let mut extension_aggregations = std::collections::HashMap::new();
        extension_aggregations.insert(extension_id.clone(), target);
        let key = ExtensionBzlTransitiveDigestKey {
            workspace_id: workspace_id.clone(),
            extension_id: Arc::from(extension_id.as_str()),
            allow_missing_loads: false,
        };

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            Arc::new(
                BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(extension_aggregations),
                ),
            ),
        )])?;
        let mut dice = updater.commit().await;
        let err = dice.compute(&key).await?.unwrap_err();

        assert!(
            err.to_string()
                .contains("requires the module extension executor"),
            "{err:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn extension_id_by_canonical_repo_key_projects_owner_extension() -> slug_error::Result<()>
    {
        fn aggregations_value(
            workspace_id: crate::WorkspaceId,
            target: AggregatedExtension,
            other: AggregatedExtension,
        ) -> Arc<BzlmodExtensionAggregationsDataValue> {
            let mut extension_aggregations = std::collections::HashMap::new();
            extension_aggregations.insert(target.extension_id.clone(), target);
            extension_aggregations.insert(other.extension_id.clone(), other);
            Arc::new(
                BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                    workspace_id,
                    "root".to_owned(),
                    Arc::new(extension_aggregations),
                ),
            )
        }

        let workspace_id =
            crate::WorkspaceId::for_project_root(PathBuf::from("/tmp/slug-plan61-canonical-key"));
        let mut target = AggregatedExtension::new("@root//:ext.bzl", "ext");
        target.extension_id = "@root//:ext.bzl%ext".to_owned();
        let target_id = target.extension_id.clone();
        let mut other = AggregatedExtension::new("@dep//:other.bzl", "other");
        other.extension_id = "@dep//:other.bzl%other".to_owned();
        let key =
            ExtensionIdByCanonicalRepoKey::for_workspace_id(workspace_id.clone(), "_main+ext+tool");

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            aggregations_value(workspace_id.clone(), target.clone(), other.clone()),
        )])?;
        let mut dice = updater.commit().await;
        let first = dice.compute(&key).await??.unwrap();
        assert_eq!(first.as_ref(), target_id);

        let mut changed_other = other.clone();
        changed_other.add_imported_repos(["changed_other_repo".to_owned()]);
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            aggregations_value(workspace_id.clone(), target.clone(), changed_other),
        )])?;
        let mut dice = updater.commit().await;
        let second = dice.compute(&key).await??.unwrap();
        assert_eq!(first, second);

        let mut changed_target = target.clone();
        changed_target.extension_name = "renamed".to_owned();
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            aggregations_value(workspace_id.clone(), changed_target, other),
        )])?;
        let mut dice = updater.commit().await;
        let third = dice.compute(&key).await??;
        assert!(third.is_none());

        Ok(())
    }

    #[test]
    fn extension_execution_key_from_aggregation_uses_repo_env_and_lockfile_data()
    -> slug_error::Result<()> {
        use crate::BzlmodExtensionAggregationsDataValue;
        use crate::BzlmodLockfileInputsValue;
        use crate::BzlmodRepoMappingsDataValue;
        use crate::WorkspaceId;
        use crate::extensions::AggregatedExtension;

        let project_root = PathBuf::from("/tmp/slug-plan61-replay-data");
        let hidden_lockfile_path = project_root.join("buck-out/v2/MODULE.bazel.lock");
        let mut repo_env = BTreeMap::new();
        repo_env.insert("TOKEN".to_owned(), "from-replay-data".to_owned());
        let workspace_id = WorkspaceId::for_project_root(project_root.clone());
        let mut data = BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
            workspace_id.clone(),
            "root".to_owned(),
            Arc::new(std::collections::HashMap::new()),
        );
        let mut aggregated = AggregatedExtension::new("@root//:ext.bzl", "ext");
        aggregated.extension_id = "@root//:ext.bzl%ext".to_owned();
        let extension_id = aggregated.extension_id.clone();
        Arc::get_mut(&mut data.extension_aggregations)
            .unwrap()
            .insert(extension_id.clone(), aggregated);
        let lockfile_inputs = BzlmodLockfileInputsValue {
            hidden_lockfile_path: Some(hidden_lockfile_path.clone()),
            visible_lockfile_digest: Some("visible-digest".to_owned()),
            hidden_lockfile_digest: Some("hidden-digest".to_owned()),
            visible_lockfile: None,
            hidden_lockfile: Some(Arc::new(LockfileContentValue {
                path: Arc::new(hidden_lockfile_path.clone()),
                digest: Some("hidden-digest".to_owned()),
                tracked_by_dice: true,
                lockfile: None,
            })),
            lockfile_mode: LockfileMode::Error,
        };
        let repo_mappings = BzlmodRepoMappingsDataValue::for_workspace(
            workspace_id.clone(),
            Arc::new(crate::RepoMappingSnapshot::new()),
            Arc::new(crate::RepoMappingOverrides::new()),
        );

        let aggregation = BzlmodExtensionAggregationValue {
            workspace_id: workspace_id.clone(),
            extension_id: Arc::from(extension_id.as_str()),
            aggregated: Arc::new(data.extension_aggregations[&extension_id].clone()),
            root_module_name: Arc::from("root"),
        };
        let replay_inputs = ModuleExtensionReplayInputsValue::from_lockfile_inputs(
            &extension_id,
            "digest-from-dice-key",
            compute_extension_input_hash(aggregation.aggregated.as_ref()).as_str(),
            Some(project_root.as_path()),
            &aggregation.root_module_name,
            &repo_env,
            repo_mappings.repo_mappings.as_ref(),
            repo_mappings.repo_mapping_overrides.as_ref(),
            &lockfile_inputs,
        )?;
        let key = create_extension_execution_key_from_aggregation(
            &aggregation,
            &repo_env,
            replay_inputs,
            repo_mappings.repo_mappings.as_ref(),
            repo_mappings.repo_mapping_overrides.as_ref(),
            Arc::from("digest-from-dice-key"),
        );

        assert_eq!(
            key.replay_inputs
                .selected_cache_identity
                .as_ref()
                .map(|identity| identity.source),
            None
        );
        assert_eq!(key.replay_inputs.lockfile_mode, LockfileMode::Error);
        assert_eq!(
            key.replay_inputs.identity_digest(),
            ModuleExtensionReplayInputsValue::from_lockfile_inputs(
                &extension_id,
                "digest-from-dice-key",
                compute_extension_input_hash(aggregation.aggregated.as_ref()).as_str(),
                Some(project_root.as_path()),
                &aggregation.root_module_name,
                &repo_env,
                repo_mappings.repo_mappings.as_ref(),
                repo_mappings.repo_mapping_overrides.as_ref(),
                &lockfile_inputs,
            )?
            .identity_digest()
        );
        assert_eq!(key.repo_env.as_ref(), &repo_env);
        assert_eq!(key.bzl_transitive_digest.as_ref(), "digest-from-dice-key");
        assert_eq!(
            lockfile_inputs
                .hidden_lockfile
                .as_ref()
                .map(|value| value.path.as_ref()),
            Some(&hidden_lockfile_path)
        );
        Ok(())
    }

    #[tokio::test]
    async fn recorded_inputs_key_rejects_file_edit() -> slug_error::Result<()> {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let watched = temp_dir.path().join("watched.txt");
        std::fs::write(&watched, "first\n").unwrap();
        let key = ModuleExtensionRecordedInputsKey::new(
            vec![crate::lockfile::recorded_file_input(&watched).unwrap()],
            Some(Arc::new(temp_dir.path().to_path_buf())),
            Some(Arc::new(BTreeMap::new())),
            Some(Arc::new(crate::RepoMappingSnapshot::new())),
        );
        let mut dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        assert!(dice.compute(&key).await?.is_ok());
        assert!(!<ModuleExtensionRecordedInputsKey as Key>::validity(
            &Ok(())
        ));

        std::fs::write(&watched, "second\n").unwrap();
        let mut dice = dice.into_updater().commit().await;
        let validation = dice.compute(&key).await?;
        assert!(
            validation
                .as_ref()
                .err()
                .is_some_and(|reason| reason.starts_with("recorded_input_changed:FILE:")),
            "{validation:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn module_extension_replay_inputs_key_selects_visible_cache_and_facts()
    -> slug_error::Result<()> {
        let project_root = PathBuf::from("/tmp/slug-plan61-replay-inputs-key");
        let workspace_id = crate::WorkspaceId::for_project_root(project_root.clone());
        let aggregated = AggregatedExtension::new("@@mod//ext.bzl", "ext");
        let extension_id = aggregated.extension_id.clone();
        let usages_digest = compute_extension_input_hash(&aggregated);
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        let mut lockfile = crate::lockfile::Lockfile::new();
        lockfile.set_extension_cache(
            extension_id.clone(),
            "bzl-digest".to_owned(),
            usages_digest.clone(),
            &repo_specs,
        );
        lockfile.set_extension_facts(
            extension_id.clone(),
            serde_json::json!({"resource": "from-visible"}),
        );
        let visible_lockfile = Arc::new(LockfileContentValue {
            path: Arc::new(project_root.join("MODULE.bazel.lock")),
            digest: Some("visible-lockfile-digest".to_owned()),
            tracked_by_dice: true,
            lockfile: Some(Arc::new(lockfile)),
        });
        let lockfile_inputs = Arc::new(BzlmodLockfileInputsValue::from_values(
            None,
            Some(visible_lockfile),
            None,
            LockfileMode::Update,
        ));
        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            crate::BzlmodLockfileInputsDataKey,
            Arc::new(crate::BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                lockfile_inputs,
            )),
        )])?;
        let mut dice = updater.commit().await;

        let replay_inputs = dice
            .compute(&ModuleExtensionReplayInputsKey {
                workspace_id,
                extension_id: Arc::from(extension_id.as_str()),
                bzl_transitive_digest: Arc::from("bzl-digest"),
                usages_digest: Arc::from(usages_digest.as_str()),
                project_root: Some(Arc::new(project_root)),
                root_module_name: Arc::from("_main"),
                repo_env: Arc::new(BTreeMap::new()),
                repo_mappings: Arc::new(crate::RepoMappingSnapshot::new()),
                repo_mapping_overrides: Arc::new(crate::RepoMappingOverrides::new()),
            })
            .await??;

        assert_eq!(
            replay_inputs
                .prior_facts
                .get("resource")
                .and_then(serde_json::Value::as_str),
            Some("from-visible")
        );
        assert!(replay_inputs.selected_cache.is_some());
        assert_eq!(
            replay_inputs
                .selected_cache_identity
                .as_ref()
                .map(|identity| identity.source),
            Some(crate::dice_graph::LockfileContentKind::Workspace)
        );
        Ok(())
    }

    #[test]
    fn extension_execution_result_validity_does_not_poll_recorded_inputs() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let watched = temp_dir.path().join("watched.txt");
        std::fs::write(&watched, "first\n").unwrap();
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));
        let result = ModuleExtensionResult::new_with_metadata_and_recorded_input_context(
            Arc::from("@@root//:ext.bzl%ext"),
            "usages-digest".to_owned(),
            repo_specs,
            "_main",
            ModuleExtensionMetadata::default(),
            vec![
                crate::lockfile::recorded_file_input_with_recorded_path(
                    PathBuf::from("@@//watched.txt").as_path(),
                    &watched,
                )
                .unwrap(),
            ],
            ModuleExtensionRecordedInputContext::new(
                Some(temp_dir.path().to_path_buf()),
                BTreeMap::new(),
                crate::RepoMappingSnapshot::new(),
            ),
        );
        let value = Ok(Arc::new(result));

        assert!(<ModuleExtensionExecutionKey as Key>::validity(&value));
        std::fs::write(&watched, "second\n").unwrap();
        assert!(<ModuleExtensionExecutionKey as Key>::validity(&value));
    }

    #[test]
    fn extension_spokes_validity_does_not_poll_recorded_inputs() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let watched = temp_dir.path().join("watched.txt");
        std::fs::write(&watched, "first\n").unwrap();
        let workspace_id = crate::WorkspaceId::for_project_root(temp_dir.path().to_path_buf());
        let value = Ok(Arc::new(ExtensionSpokesValue {
            workspace_id: workspace_id.clone(),
            extension_id: Arc::from("@@root//:ext.bzl%ext"),
            bzl_transitive_digest: Arc::from("digest"),
            usages_digest: Arc::from("usages"),
            replay_inputs_identity_digest: Arc::from("replay"),
            repo_mappings_digest: Arc::from("repo-mappings"),
            repo_mapping_overrides_digest: Arc::from("repo-mapping-overrides"),
            project_root: workspace_id.canonical_project_root.clone(),
            repo_env: Arc::new(BTreeMap::new()),
            spokes: BTreeMap::new(),
            recorded_inputs: Arc::new(vec![
                crate::lockfile::recorded_file_input_with_recorded_path(
                    PathBuf::from("@@//watched.txt").as_path(),
                    &watched,
                )
                .unwrap(),
            ]),
            recorded_input_workspace_root: Some(Arc::new(temp_dir.path().to_path_buf())),
            recorded_input_repo_env: Arc::new(BTreeMap::new()),
            recorded_input_repo_mappings: Arc::new(crate::RepoMappingSnapshot::new()),
        }));
        let optional_value = Ok(Some(value.as_ref().unwrap().clone()));

        assert!(<ExtensionSpokesKey as Key>::validity(&value));
        assert!(!<ExtensionSpokesByExtensionIdKey as Key>::validity(
            &optional_value
        ));
        assert!(!<ExtensionSpokesByCanonicalRepoKey as Key>::validity(
            &optional_value
        ));

        std::fs::write(&watched, "second\n").unwrap();
        assert!(<ExtensionSpokesKey as Key>::validity(&value));
        assert!(!<ExtensionSpokesByExtensionIdKey as Key>::validity(
            &optional_value
        ));
        assert!(!<ExtensionSpokesByCanonicalRepoKey as Key>::validity(
            &optional_value
        ));
    }

    #[tokio::test]
    async fn visible_lockfile_replay_validates_recorded_file_through_dice_key()
    -> slug_error::Result<()> {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();
        let watched = project_root.join("watched.txt");
        std::fs::write(&watched, "first\n").unwrap();

        let aggregated = AggregatedExtension::new("@root//:ext.bzl", "ext");
        let extension_id = aggregated.extension_id.clone();
        let usages_digest = compute_extension_input_hash(&aggregated);
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));

        let mut lockfile = crate::lockfile::Lockfile::new();
        lockfile.set_extension_cache(
            extension_id.clone(),
            "bzl-digest".to_owned(),
            usages_digest,
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut(&extension_id)
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push(crate::lockfile::recorded_file_input(&watched).unwrap());
        let visible_lockfile = Arc::new(LockfileContentValue {
            path: Arc::new(project_root.join("MODULE.bazel.lock")),
            digest: Some("visible-lockfile-digest".to_owned()),
            tracked_by_dice: true,
            lockfile: Some(Arc::new(lockfile)),
        });
        let workspace_id = crate::WorkspaceId::for_project_root(project_root.clone());
        let key = ModuleExtensionExecutionKey::new_with_tracked_lockfiles_and_bzl_digest(
            aggregated,
            "_main".to_owned(),
            project_root,
            Some("visible-lockfile-digest".to_owned()),
            None,
            Some(visible_lockfile),
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
            Arc::from("bzl-digest"),
            workspace_id,
        );
        let mut dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await??;
        assert!(first.contains_repo("repo"));

        std::fs::write(&watched, "second\n").unwrap();
        let mut dice = dice.into_updater().commit().await;
        let err = dice.compute(&key).await?.unwrap_err();
        assert!(
            err.to_string()
                .contains("module extension executor is not initialized"),
            "{err:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn hidden_lockfile_replay_validates_recorded_file_through_dice_key()
    -> slug_error::Result<()> {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();
        let watched = project_root.join("watched.txt");
        std::fs::write(&watched, "first\n").unwrap();

        let aggregated = AggregatedExtension::new("@root//:ext.bzl", "ext");
        let extension_id = aggregated.extension_id.clone();
        let usages_digest = compute_extension_input_hash(&aggregated);
        let mut repo_specs = FxHashMap::default();
        repo_specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));

        let mut lockfile = crate::lockfile::Lockfile::new();
        lockfile.set_extension_cache(
            extension_id.clone(),
            "bzl-digest".to_owned(),
            usages_digest,
            &repo_specs,
        );
        lockfile
            .module_extensions
            .get_mut(&extension_id)
            .unwrap()
            .general
            .as_mut()
            .unwrap()
            .recorded_inputs
            .push(crate::lockfile::recorded_file_input(&watched).unwrap());
        let hidden_lockfile = Arc::new(LockfileContentValue {
            path: Arc::new(project_root.join("buck-out/v2/MODULE.bazel.lock")),
            digest: Some("hidden-lockfile-digest".to_owned()),
            tracked_by_dice: true,
            lockfile: Some(Arc::new(lockfile)),
        });
        let workspace_id = crate::WorkspaceId::for_project_root(project_root.clone());
        let key = ModuleExtensionExecutionKey::new_with_tracked_lockfiles_and_bzl_digest(
            aggregated,
            "_main".to_owned(),
            project_root.clone(),
            None,
            Some("hidden-lockfile-digest".to_owned()),
            None,
            Some(hidden_lockfile),
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
            Arc::from("bzl-digest"),
            workspace_id,
        );
        let mut dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = dice.compute(&key).await??;
        assert!(first.contains_repo("repo"));

        std::fs::write(&watched, "second\n").unwrap();
        let mut dice = dice.into_updater().commit().await;
        let err = dice.compute(&key).await?.unwrap_err();
        assert!(
            err.to_string()
                .contains("module extension executor is not initialized"),
            "{err:?}"
        );

        Ok(())
    }

    #[test]
    fn test_sanitize_extension_id() {
        assert_eq!(
            sanitize_extension_id_for_path("@@module//path:file.bzl%ext"),
            "__module__path_file.bzl_ext"
        );
        assert_eq!(sanitize_extension_id_for_path("simple_name"), "simple_name");
        assert_eq!(
            sanitize_extension_id_for_path("name with spaces"),
            "name_with_spaces"
        );
    }

    #[test]
    fn test_module_extension_key_creation() {
        use crate::extensions::AggregatedExtension;

        let mut aggregated = AggregatedExtension::new("@@module//ext.bzl", "test");
        aggregated.add_module_tags("root", vec![]);

        let key = ModuleExtensionExecutionKey::new(aggregated, "_main".to_owned());

        assert_eq!(key.extension_id.as_ref(), "@@module//ext.bzl%test");
        assert_eq!(
            base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                key.input_hash.as_ref()
            )
            .unwrap()
            .len(),
            32
        );
        assert_eq!(key.root_module_name.as_ref(), "_main");
    }

    #[test]
    fn test_module_extension_key_minimal() {
        let key = ModuleExtensionExecutionKey::new_minimal(
            "@@module//ext.bzl%test".to_owned(),
            "sha256-abc".to_owned(),
        );

        assert_eq!(key.extension_id.as_ref(), "@@module//ext.bzl%test");
        assert_eq!(key.input_hash.as_ref(), "sha256-abc");
        assert_eq!(key.root_module_name.as_ref(), "_main");
    }

    #[test]
    fn test_module_extension_key_display() {
        let key = ModuleExtensionExecutionKey::new_minimal(
            "@@m//e.bzl%x".to_owned(),
            "hash123".to_owned(),
        );

        let display = format!("{}", key);
        assert!(display.starts_with("ModuleExtensionKey(@@m//e.bzl%x, hash123, "));
    }

    #[test]
    fn test_module_extension_key_with_tags() {
        use crate::extensions::AggregatedExtension;
        use crate::types::ExtensionTag;
        use crate::types::TagValue;

        let mut aggregated = AggregatedExtension::new("@@rules_python//pip:pip.bzl", "pip");

        let mut parse_tag = ExtensionTag::new("parse".to_owned());
        parse_tag
            .kwargs
            .push(("hub_name".to_owned(), TagValue::String("pip".to_owned())));

        let mut install_tag = ExtensionTag::new("install".to_owned());
        install_tag
            .kwargs
            .push(("name".to_owned(), TagValue::String("numpy".to_owned())));

        aggregated.add_module_tags("root", vec![parse_tag]);
        aggregated.add_module_tags("dep_a", vec![install_tag]);

        let key = ModuleExtensionExecutionKey::new(aggregated, "root".to_owned());

        assert_eq!(key.extension_id.as_ref(), "@@rules_python//pip:pip.bzl%pip");
        assert_eq!(key.root_module_name.as_ref(), "root");
        assert_eq!(key.aggregated().tags_by_module.len(), 2);
    }

    #[test]
    fn test_module_extension_key_hash_eq() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

        use crate::extensions::AggregatedExtension;

        let aggregated1 = AggregatedExtension::new("@@mod//ext.bzl", "ext");
        let aggregated2 = AggregatedExtension::new("@@mod//ext.bzl", "ext");

        let key1 = ModuleExtensionExecutionKey::new(aggregated1, "_main".to_owned());
        let key2 = ModuleExtensionExecutionKey::new(aggregated2, "_main".to_owned());

        // Keys with same aggregated data should be equal
        assert_eq!(key1, key2);

        // Keys with same aggregated data should have same hash
        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        key2.hash(&mut hasher2);
        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_get_repo_spec() {
        let mut specs = FxHashMap::default();
        specs.insert(
            "test_repo".to_owned(),
            RepoSpec::new("@@bazel_tools//repo:http.bzl%http_archive".to_owned()).with_attr(
                "url".to_owned(),
                AttrValue::String("https://example.com".to_owned()),
            ),
        );

        let result =
            ModuleExtensionResult::new(Arc::from("@@//ext.bzl%test"), "hash".to_owned(), specs, "");

        let spec = result.get_repo_spec("test_repo").unwrap();
        assert_eq!(
            spec.repo_rule_id,
            "@@bazel_tools//repo:http.bzl%http_archive"
        );
        assert!(result.get_repo_spec("nonexistent").is_none());
    }

    #[test]
    fn test_module_extension_result_carries_facts_metadata() {
        let mut specs = FxHashMap::default();
        specs.insert("repo".to_owned(), RepoSpec::new("rule".to_owned()));

        let result = ModuleExtensionResult::new_with_metadata(
            Arc::from("@@//ext.bzl%test"),
            "hash".to_owned(),
            specs,
            "",
            ModuleExtensionMetadata {
                facts: serde_json::json!({"resource": {"checksum": "abc"}}),
            },
            vec!["ENV:PLAN61_ENV value".to_owned()],
        );

        assert_eq!(
            result.metadata.facts,
            serde_json::json!({"resource": {"checksum": "abc"}})
        );
        assert_eq!(result.recorded_inputs, vec!["ENV:PLAN61_ENV value"]);
    }

    #[test]
    fn test_error_mode_facts_accepts_matching_workspace_facts() {
        validate_error_mode_facts(
            "@@mod+//:ext.bzl%ext",
            LockfileMode::Error,
            &serde_json::json!({"resource": {"checksum": "abc"}}),
            &serde_json::json!({"resource": {"checksum": "abc"}}),
        )
        .unwrap();
    }

    #[test]
    fn test_error_mode_facts_rejects_changed_workspace_facts() {
        let err = validate_error_mode_facts(
            "@@mod+//:ext.bzl%ext",
            LockfileMode::Error,
            &serde_json::json!({"resource": {"checksum": "new"}}),
            &serde_json::json!({"resource": {"checksum": "old"}}),
        )
        .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("MODULE.bazel.lock is no longer up-to-date"));
        assert!(message.contains("the extension '@@mod+//:ext.bzl%ext' has changed its facts"));
        assert!(message.contains(r#""checksum":"new""#));
        assert!(message.contains(r#""checksum":"old""#));
        assert!(message.contains("bazel mod deps --lockfile_mode=update"));
    }

    #[test]
    fn test_error_mode_facts_rejects_new_facts_when_workspace_facts_absent() {
        let err = validate_error_mode_facts(
            "@@mod+//:ext.bzl%ext",
            LockfileMode::Error,
            &serde_json::json!({"resource": "new"}),
            &empty_facts(),
        )
        .unwrap_err();

        assert!(err.to_string().contains(r#"{"resource":"new"} != {}"#));
    }

    #[test]
    fn test_repo_names_iterator() {
        let mut specs = FxHashMap::default();
        specs.insert("a".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("b".to_owned(), RepoSpec::new("rule".to_owned()));
        specs.insert("c".to_owned(), RepoSpec::new("rule".to_owned()));

        let result =
            ModuleExtensionResult::new(Arc::from("@@//ext.bzl%test"), "hash".to_owned(), specs, "");

        let mut names: Vec<_> = result.repo_names().collect();
        names.sort();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    // =========================================================================
    // Lockfile Integration Tests
    // =========================================================================

    #[test]
    fn test_compute_bzl_transitive_digest() {
        let digest1 = compute_bzl_transitive_digest("@@module//ext.bzl%test");
        let digest2 = compute_bzl_transitive_digest("@@module//ext.bzl%test");
        let digest3 = compute_bzl_transitive_digest("@@other//ext.bzl%test");

        // Same extension ID should produce same digest
        assert_eq!(digest1, digest2);
        // Different extension ID should produce different digest
        assert_ne!(digest1, digest3);
        assert_eq!(
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, digest1)
                .unwrap()
                .len(),
            32
        );
    }

    #[test]
    fn test_lockfile_project_root_reads_without_writing() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let lock_path = temp_dir.path().join("MODULE.bazel.lock");
        let mut lockfile = crate::lockfile::Lockfile::new();
        lockfile.registry_file_hashes.insert(
            "https://bcr.bazel.build/test".to_owned(),
            "sha256-abc".to_owned(),
        );
        lockfile.write(&lock_path).unwrap();
        let before = std::fs::read(&lock_path).unwrap();

        let cached =
            crate::lockfile::read_lockfile_with_mode(temp_dir.path(), LockfileMode::Update)
                .unwrap()
                .unwrap();

        assert_eq!(cached.registry_file_hashes.len(), 1);
        assert_eq!(std::fs::read(&lock_path).unwrap(), before);
    }

    #[test]
    fn test_new_with_lockfile_constructor() {
        use crate::extensions::AggregatedExtension;

        let aggregated = AggregatedExtension::new("@@module//ext.bzl", "test");
        let key = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated,
            "_main".to_owned(),
            PathBuf::from("/tmp/project"),
            None,
            None,
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );

        assert_eq!(key.extension_id.as_ref(), "@@module//ext.bzl%test");
        assert!(key.project_root.is_some());
        assert_eq!(key.project_root().unwrap(), &PathBuf::from("/tmp/project"));
        let workspace_id = &key.workspace_id;
        assert_eq!(
            workspace_id.canonical_project_root.as_ref(),
            &PathBuf::from("/tmp/project")
        );
        assert_eq!(
            workspace_id.output_base.as_ref(),
            &PathBuf::from("/tmp/project/buck-out/v2")
        );
    }

    #[test]
    fn test_extension_execution_carries_workspace_id() {
        use crate::extensions::AggregatedExtension;

        let minimal = ModuleExtensionExecutionKey::new(
            AggregatedExtension::new("@@module//ext.bzl", "test"),
            "_main".to_owned(),
        );
        assert_eq!(
            minimal
                .execution_workspace_id()
                .canonical_project_root
                .as_ref(),
            &PathBuf::from("__test__")
        );

        let present = ModuleExtensionExecutionKey::new_with_lockfile(
            AggregatedExtension::new("@@module//ext.bzl", "test"),
            "_main".to_owned(),
            PathBuf::from("/tmp/project"),
            None,
            None,
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );
        assert_eq!(
            present
                .execution_workspace_id()
                .canonical_project_root
                .as_ref(),
            &PathBuf::from("/tmp/project")
        );
    }

    #[test]
    fn test_project_root_is_in_hash_and_eq() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

        use crate::extensions::AggregatedExtension;

        let aggregated1 = AggregatedExtension::new("@@mod//ext.bzl", "ext");
        let aggregated2 = AggregatedExtension::new("@@mod//ext.bzl", "ext");

        let key1 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated1,
            "_main".to_owned(),
            PathBuf::from("/project1"),
            None,
            None,
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );
        let key2 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated2,
            "_main".to_owned(),
            PathBuf::from("/project2"),
            None,
            None,
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );

        assert_ne!(key1, key2);

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        key2.hash(&mut hasher2);
        assert_ne!(hasher1.finish(), hasher2.finish());

        let mut key3 = ModuleExtensionExecutionKey::new_with_lockfile(
            AggregatedExtension::new("@@mod//ext.bzl", "ext"),
            "_main".to_owned(),
            PathBuf::from("/project1"),
            None,
            None,
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );
        key3.workspace_id = crate::WorkspaceId::new(
            PathBuf::from("/project1"),
            PathBuf::from("/alternate-output-base"),
        );

        assert_ne!(key1, key3);

        let mut hasher1 = DefaultHasher::new();
        let mut hasher3 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        key3.hash(&mut hasher3);
        assert_ne!(hasher1.finish(), hasher3.finish());
    }

    #[test]
    fn test_project_bzl_digest_is_in_hash_and_eq() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

        use crate::extensions::AggregatedExtension;

        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("ext.bzl"),
            "def _impl(ctx):\n    pass\n",
        )
        .unwrap();
        let aggregated1 = AggregatedExtension::new("@@mod//:ext.bzl", "ext");
        let key1 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated1,
            "_main".to_owned(),
            temp_dir.path().to_path_buf(),
            None,
            None,
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );

        std::fs::write(
            temp_dir.path().join("ext.bzl"),
            "def _impl(ctx):\n    fail('changed')\n",
        )
        .unwrap();
        let aggregated2 = AggregatedExtension::new("@@mod//:ext.bzl", "ext");
        let key2 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated2,
            "_main".to_owned(),
            temp_dir.path().to_path_buf(),
            None,
            None,
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );

        assert_ne!(key1.bzl_transitive_digest, key2.bzl_transitive_digest);
        assert_ne!(key1, key2);

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        key2.hash(&mut hasher2);
        assert_ne!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_project_bzl_digest_includes_existing_external_loads() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("ext.bzl"),
            "load(\"@helper_repo//tools:helper.bzl\", \"HELPER\")\n",
        )
        .unwrap();
        let helper_dir = temp_dir.path().join("bazel-external/helper_repo/tools");
        std::fs::create_dir_all(&helper_dir).unwrap();
        let helper_path = helper_dir.join("helper.bzl");
        std::fs::write(&helper_path, "HELPER = \"first\"\n").unwrap();

        let first =
            compute_bzl_transitive_digest_for_project("@@mod//:ext.bzl%ext", temp_dir.path());

        std::fs::write(&helper_path, "HELPER = \"second\"\n").unwrap();
        let second =
            compute_bzl_transitive_digest_for_project("@@mod//:ext.bzl%ext", temp_dir.path());

        assert_ne!(first, second);

        std::fs::remove_file(&helper_path).unwrap();
        let deleted =
            compute_bzl_transitive_digest_for_project("@@mod//:ext.bzl%ext", temp_dir.path());

        assert_ne!(second, deleted);
    }

    #[test]
    fn test_project_bzl_digest_includes_missing_project_load_state() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("ext.bzl"),
            "load(\"//tools:helper.bzl\", \"HELPER\")\n",
        )
        .unwrap();
        let helper_dir = temp_dir.path().join("tools");
        std::fs::create_dir_all(&helper_dir).unwrap();
        let helper_path = helper_dir.join("helper.bzl");

        let missing =
            compute_bzl_transitive_digest_for_project("@@mod//:ext.bzl%ext", temp_dir.path());

        std::fs::write(&helper_path, "HELPER = \"created\"\n").unwrap();
        let created =
            compute_bzl_transitive_digest_for_project("@@mod//:ext.bzl%ext", temp_dir.path());

        assert_ne!(missing, created);
    }

    #[test]
    fn test_project_bzl_digest_preserves_canonical_external_plus_repo() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp_dir.path().join("ext.bzl"),
            "load(\"@rules_python+//python:defs.bzl\", \"PY\")\n",
        )
        .unwrap();
        let helper_dir = temp_dir.path().join("bazel-external/rules_python+/python");
        std::fs::create_dir_all(&helper_dir).unwrap();
        let helper_path = helper_dir.join("defs.bzl");
        std::fs::write(&helper_path, "PY = \"first\"\n").unwrap();

        let first =
            compute_bzl_transitive_digest_for_project("@@mod//:ext.bzl%ext", temp_dir.path());

        std::fs::write(&helper_path, "PY = \"second\"\n").unwrap();
        let second =
            compute_bzl_transitive_digest_for_project("@@mod//:ext.bzl%ext", temp_dir.path());

        assert_ne!(first, second);
    }

    #[test]
    fn test_project_bzl_digest_resolves_mapped_apparent_external_loads() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let owner_dir = temp_dir.path().join("bazel-external/rules_owner+");
        std::fs::create_dir_all(&owner_dir).unwrap();
        std::fs::write(
            owner_dir.join("ext.bzl"),
            "load(\"@apparent_helper//:helper.bzl\", \"HELPER\")\n",
        )
        .unwrap();

        let apparent_dir = temp_dir.path().join("bazel-external/apparent_helper");
        std::fs::create_dir_all(&apparent_dir).unwrap();
        let apparent_helper = apparent_dir.join("helper.bzl");
        std::fs::write(&apparent_helper, "HELPER = \"wrong repo\"\n").unwrap();

        let real_dir = temp_dir.path().join("bazel-external/real_helper+");
        std::fs::create_dir_all(&real_dir).unwrap();
        let real_helper = real_dir.join("helper.bzl");
        std::fs::write(&real_helper, "HELPER = \"first\"\n").unwrap();

        let wrong_dir = temp_dir.path().join("bazel-external/wrong_helper+");
        std::fs::create_dir_all(&wrong_dir).unwrap();
        let wrong_helper = wrong_dir.join("helper.bzl");
        std::fs::write(&wrong_helper, "HELPER = \"wrong mapped repo\"\n").unwrap();

        let mut source_mapping = BTreeMap::new();
        source_mapping.insert("apparent_helper".to_owned(), "real_helper".to_owned());
        let mut fallback_source_mapping = BTreeMap::new();
        fallback_source_mapping.insert("apparent_helper".to_owned(), "wrong_helper".to_owned());
        let mut repo_mappings = crate::RepoMappingSnapshot::new();
        repo_mappings.insert("rules_owner+".to_owned(), source_mapping);
        repo_mappings.insert("rules_owner".to_owned(), fallback_source_mapping);

        let first = compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
            "@@rules_owner+//:ext.bzl%ext",
            temp_dir.path(),
            Some(&repo_mappings),
        );

        std::fs::write(&apparent_helper, "HELPER = \"still wrong repo\"\n").unwrap();
        let after_apparent_edit =
            compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
                "@@rules_owner+//:ext.bzl%ext",
                temp_dir.path(),
                Some(&repo_mappings),
            );
        assert_eq!(first, after_apparent_edit);

        std::fs::write(&wrong_helper, "HELPER = \"still wrong mapped repo\"\n").unwrap();
        let after_unsuffixed_mapping_edit =
            compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
                "@@rules_owner+//:ext.bzl%ext",
                temp_dir.path(),
                Some(&repo_mappings),
            );
        assert_eq!(first, after_unsuffixed_mapping_edit);

        std::fs::write(&real_helper, "HELPER = \"second\"\n").unwrap();
        let after_real_edit =
            compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
                "@@rules_owner+//:ext.bzl%ext",
                temp_dir.path(),
                Some(&repo_mappings),
            );

        assert_ne!(first, after_real_edit);
    }

    #[test]
    fn test_project_bzl_digest_includes_missing_mapped_external_load_state() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let owner_dir = temp_dir.path().join("bazel-external/rules_owner+");
        std::fs::create_dir_all(&owner_dir).unwrap();
        std::fs::write(
            owner_dir.join("ext.bzl"),
            "load(\"@apparent_helper//:helper.bzl\", \"HELPER\")\n",
        )
        .unwrap();

        let real_dir = temp_dir.path().join("bazel-external/real_helper+");
        std::fs::create_dir_all(&real_dir).unwrap();
        let real_helper = real_dir.join("helper.bzl");

        let mut source_mapping = BTreeMap::new();
        source_mapping.insert("apparent_helper".to_owned(), "real_helper".to_owned());
        let mut repo_mappings = crate::RepoMappingSnapshot::new();
        repo_mappings.insert("rules_owner+".to_owned(), source_mapping);

        let missing = compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
            "@@rules_owner+//:ext.bzl%ext",
            temp_dir.path(),
            Some(&repo_mappings),
        );

        std::fs::write(&real_helper, "HELPER = \"created\"\n").unwrap();
        let created = compute_fallback_scanned_bzl_transitive_digest_for_project_with_repo_mappings(
            "@@rules_owner+//:ext.bzl%ext",
            temp_dir.path(),
            Some(&repo_mappings),
        );

        assert_ne!(missing, created);
    }

    #[test]
    fn test_from_arcs_with_lockfile() {
        use crate::extensions::AggregatedExtension;

        let extension_id = Arc::from("@@mod//ext.bzl%ext");
        let input_hash = Arc::from("sha256-abc");
        let aggregated = Arc::new(AggregatedExtension::new("@@mod//ext.bzl", "ext"));
        let root_module_name = Arc::from("_main");
        let project_root = Arc::new(PathBuf::from("/tmp/test"));

        let key = ModuleExtensionExecutionKey::from_arcs_with_lockfile(
            extension_id,
            input_hash,
            aggregated,
            root_module_name,
            project_root,
            None,
            None,
            LockfileMode::Update,
            Arc::new(BTreeMap::new()),
            Arc::new(crate::RepoMappingSnapshot::new()),
            Arc::new(crate::RepoMappingOverrides::new()),
        );

        assert!(key.project_root.is_some());
        assert_eq!(key.project_root().unwrap(), &PathBuf::from("/tmp/test"));
    }

    #[test]
    fn test_replay_inputs_identity_is_in_hash_and_eq() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

        use crate::extensions::AggregatedExtension;

        fn visible_lockfile_with_fact(marker: &str) -> Arc<LockfileContentValue> {
            let mut lockfile = crate::lockfile::Lockfile::new();
            lockfile.set_extension_facts(
                "@@mod//ext.bzl%ext".to_owned(),
                serde_json::json!({ "marker": marker }),
            );
            Arc::new(LockfileContentValue {
                path: Arc::new(PathBuf::from("/project/MODULE.bazel.lock")),
                digest: Some(format!("visible-digest-{marker}")),
                tracked_by_dice: true,
                lockfile: Some(Arc::new(lockfile)),
            })
        }

        fn key_with_visible_lockfile(
            visible_lockfile: Arc<LockfileContentValue>,
        ) -> ModuleExtensionExecutionKey {
            let project_root = PathBuf::from("/project");
            let workspace_id = crate::WorkspaceId::for_project_root(project_root.clone());
            ModuleExtensionExecutionKey::new_with_tracked_lockfiles_and_bzl_digest(
                AggregatedExtension::new("@@mod//ext.bzl", "ext"),
                "_main".to_owned(),
                project_root,
                visible_lockfile.digest.clone(),
                None,
                Some(visible_lockfile),
                None,
                LockfileMode::Update,
                BTreeMap::new(),
                crate::RepoMappingSnapshot::new(),
                crate::RepoMappingOverrides::new(),
                Arc::from("bzl-digest"),
                workspace_id,
            )
        }

        let first = key_with_visible_lockfile(visible_lockfile_with_fact("first"));
        let second = key_with_visible_lockfile(visible_lockfile_with_fact("second"));

        assert_ne!(first, second);

        let mut first_hasher = DefaultHasher::new();
        let mut second_hasher = DefaultHasher::new();
        first.hash(&mut first_hasher);
        second.hash(&mut second_hasher);
        assert_ne!(first_hasher.finish(), second_hasher.finish());
    }

    #[test]
    fn test_lockfile_mode_is_in_hash_and_eq() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;
        use std::hash::Hasher;

        use crate::extensions::AggregatedExtension;

        let aggregated1 = AggregatedExtension::new("@@mod//ext.bzl", "ext");
        let aggregated2 = AggregatedExtension::new("@@mod//ext.bzl", "ext");

        let key1 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated1,
            "_main".to_owned(),
            PathBuf::from("/project"),
            None,
            None,
            None,
            LockfileMode::Update,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );
        let key2 = ModuleExtensionExecutionKey::new_with_lockfile(
            aggregated2,
            "_main".to_owned(),
            PathBuf::from("/project"),
            None,
            None,
            None,
            LockfileMode::Off,
            BTreeMap::new(),
            crate::RepoMappingSnapshot::new(),
            crate::RepoMappingOverrides::new(),
        );

        assert_ne!(key1, key2);

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        key2.hash(&mut hasher2);
        assert_ne!(hasher1.finish(), hasher2.finish());
    }

    #[test]
    fn test_key_without_lockfile_has_no_project_root() {
        use crate::extensions::AggregatedExtension;

        let aggregated = AggregatedExtension::new("@@mod//ext.bzl", "ext");
        let key = ModuleExtensionExecutionKey::new(aggregated, "_main".to_owned());

        assert!(key.project_root.is_none());
        assert!(key.project_root().is_none());
    }
}
