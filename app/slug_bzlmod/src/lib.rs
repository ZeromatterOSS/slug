/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bzlmod (Bazel module) implementation for Slug.
//!
//! This crate provides parsing and resolution of MODULE.bazel files,
//! implementing Bazel 9.0's module system for dependency management.
//!
//! # Components
//!
//! - [`types`]: Data structures for Module, BazelDep, and related types
//! - [`version`]: Bazel-compatible version parsing and comparison
//! - [`globals`]: Starlark globals for MODULE.bazel directives
//! - [`parser`]: MODULE.bazel file parsing
//! - [`cache`]: Module caching for fetched dependencies
//! - [`registry`]: Bazel Central Registry (BCR) client
//! - [`fetch`]: Source fetching and extraction
//! - [`integrity`]: Subresource Integrity (SRI) hash verification
//! - [`resolution`]: Module resolution with MVS algorithm
//! - [`lockfile`]: MODULE.bazel.lock file handling

pub mod cache;
pub mod dice_graph;
mod extension_execution_dice;
mod extensions;
pub mod fetch;
pub mod globals;
pub mod integrity;
mod lockfile;
mod module_extension_executor;
mod module_names;
mod parser;
pub mod pending_repo_cells;
pub mod registry;
pub mod repo_mapping;
mod repo_spec;
mod repository_execution;
mod repository_executor;
mod repository_invocations;
pub mod resolution;
mod spoke_materialization;
mod starlark_repo_rule_executor;
mod types;
mod version;

// ============================================================================
// Module version registry
// ============================================================================
use std::collections::BTreeMap;
use std::collections::HashMap;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
pub use cache::ModuleCache;
pub use dice_graph::BZLMOD_CLEAN_GRAPH_IO_IMPL;
pub use dice_graph::BzlmodCellGraphAlias;
pub use dice_graph::BzlmodCellGraphCell;
use dice_graph::BzlmodCellGraphDataKey;
use dice_graph::BzlmodCellGraphDataValue;
pub use dice_graph::BzlmodCellGraphDynamicAlias;
pub use dice_graph::BzlmodCellGraphExtensionCell;
pub use dice_graph::BzlmodCellGraphKey;
pub use dice_graph::BzlmodCellGraphModuleSetup;
pub use dice_graph::BzlmodCellGraphModuleSymlink;
pub use dice_graph::BzlmodCellGraphScopedAlias;
pub use dice_graph::BzlmodCellGraphValue;
pub use dice_graph::BzlmodCleanCellGraphBuilder;
pub use dice_graph::BzlmodCleanGraphIo;
pub use dice_graph::BzlmodCleanLockfileInputsKey;
pub use dice_graph::BzlmodCommandPolicyKey;
pub use dice_graph::BzlmodCommandPolicyValue;
use dice_graph::BzlmodCurrentCellGraphKey;
pub use dice_graph::BzlmodEventCounters;
pub use dice_graph::BzlmodEventKind;
pub use dice_graph::BzlmodExtensionAggregationKey;
pub use dice_graph::BzlmodExtensionAggregationValue;
pub use dice_graph::BzlmodExtensionAggregationsDataKey;
pub use dice_graph::BzlmodExtensionAggregationsDataValue;
pub use dice_graph::BzlmodLockfileInputsDataKey;
pub use dice_graph::BzlmodLockfileInputsDataValue;
pub use dice_graph::BzlmodLockfileInputsKey;
pub use dice_graph::BzlmodLockfileInputsValue;
use dice_graph::BzlmodModuleSourcesDataKey;
use dice_graph::BzlmodModuleSourcesDataValue;
pub use dice_graph::BzlmodModuleVersionsDataKey;
pub use dice_graph::BzlmodModuleVersionsDataValue;
pub use dice_graph::BzlmodModuleVersionsInvalidation;
pub use dice_graph::BzlmodRegisteredExecutionPlatformsDataKey;
pub use dice_graph::BzlmodRegisteredToolchainsDataKey;
pub use dice_graph::BzlmodRepoEnvDataKey;
pub use dice_graph::BzlmodRepoEnvDataValue;
pub use dice_graph::BzlmodRepoEnvKey;
pub use dice_graph::BzlmodRepoMappingsDataKey;
pub use dice_graph::BzlmodRepoMappingsDataValue;
pub use dice_graph::BzlmodRepoMappingsKey;
pub use dice_graph::BzlmodResolutionFactsDataKey;
pub use dice_graph::BzlmodResolutionFactsKey;
pub use dice_graph::BzlmodResolutionFactsValue;
pub use dice_graph::BzlmodResolutionKey;
pub use dice_graph::BzlmodResolutionOptions;
pub use dice_graph::BzlmodResolvedGraphOutputsValue;
pub use dice_graph::BzlmodResolvedGraphProjectionValues;
pub use dice_graph::BzlmodResolvedGraphSourceInputsValue;
pub use dice_graph::BzlmodResolvedModuleGraphKey;
pub use dice_graph::BzlmodResolvedModuleGraphValue;
pub use dice_graph::BzlmodResolvedModuleSource;
pub use dice_graph::BzlmodWorkspaceKey;
pub use dice_graph::ExtensionBzlTransitiveDigestKey;
pub use dice_graph::ExtensionIdByCanonicalRepoKey;
pub use dice_graph::ExtensionRepoExecutionIdentity;
pub use dice_graph::ExtensionSpoke;
pub use dice_graph::ExtensionSpokesByCanonicalRepoKey;
pub use dice_graph::ExtensionSpokesByExtensionIdKey;
pub use dice_graph::ExtensionSpokesKey;
pub use dice_graph::ExtensionSpokesValue;
pub use dice_graph::ExtensionUniqueName;
pub use dice_graph::ExternalSymlinkLayoutKey;
pub use dice_graph::InnateExtensionKey;
pub use dice_graph::LocalOverrideModuleInputsValue;
pub use dice_graph::LocalOverrideSourceKey;
pub use dice_graph::LockfileContentKind;
pub use dice_graph::LockfileContentValue;
pub use dice_graph::LockfileExtensionEntryKey;
pub use dice_graph::ModuleExtensionAggregationKey;
pub use dice_graph::ModuleExtensionExecutionIdentity;
pub use dice_graph::ModuleExtensionId;
pub use dice_graph::ModuleExtensionReplayInputKey;
pub use dice_graph::ModuleFileKey;
pub use dice_graph::ModuleSourceKey;
pub use dice_graph::ModuleVersionsKey;
pub use dice_graph::ModuleVersionsValue;
pub use dice_graph::NonRegistryOverrideModuleInput;
pub use dice_graph::NonRegistryOverrideModuleInputsValue;
pub use dice_graph::NonRegistryOverrideModuleSource;
pub use dice_graph::NonRootModuleFileInput;
pub use dice_graph::NonRootModuleFilesValue;
pub use dice_graph::RegisteredExecutionPlatformsDataValue;
pub use dice_graph::RegisteredExecutionPlatformsKey;
pub use dice_graph::RegisteredExecutionPlatformsValue;
pub use dice_graph::RegisteredToolchainsDataValue;
pub use dice_graph::RegisteredToolchainsKey;
pub use dice_graph::RegisteredToolchainsValue;
pub use dice_graph::RegistryFileInputsValue;
pub use dice_graph::RepoMappingKey;
pub use dice_graph::RepoMappingScope;
pub use dice_graph::RepoMaterializationManifestKey;
pub use dice_graph::RepoMaterializationManifestValue;
pub use dice_graph::RepoOriginKind;
pub use dice_graph::ResolvedModuleIdentity;
pub use dice_graph::RootModuleFileValue;
pub use dice_graph::WorkspaceId;
pub use dice_graph::active_root_overrides;
pub use dice_graph::allow_yanked_versions_digest;
pub use dice_graph::append_resolved_non_root_modules;
pub use dice_graph::bazel_canonical_module_repo_name;
pub use dice_graph::bzlmod_event_counters;
pub use dice_graph::bzlmod_resolved_graph_digest;
pub use dice_graph::canonicalize_repo_mapping_overrides_targets;
pub use dice_graph::canonicalize_repo_mapping_snapshot_targets;
pub use dice_graph::clean_resolved_graph_outputs_value;
pub use dice_graph::collect_bzlmod_registered_items;
pub use dice_graph::graph_owned_repo_mapping_state;
pub use dice_graph::local_overrides_from_root_module;
pub use dice_graph::materialize_non_registry_override_module_input;
pub use dice_graph::module_file_inputs_digest;
pub use dice_graph::non_registry_override_module_inputs_from_root_module;
pub use dice_graph::override_patch_labels_from_root_module;
pub use dice_graph::record_bzlmod_event;
pub use dice_graph::repo_env_policy_digest;
pub use dice_graph::resolve_graph_with_module_file_inputs;
pub use dice_graph::resolved_graph_projection_values;
pub use dice_graph::resolved_module_sources_from_graph;
pub use dice_graph::selected_bzlmod_cell_name_for_dep;
pub use extension_execution_dice::ExtensionSpokesIdentityValue;
pub use extension_execution_dice::ModuleExtensionError;
pub use extension_execution_dice::ModuleExtensionExecutionKey;
pub use extension_execution_dice::ModuleExtensionRecordedInputsKey;
pub use extension_execution_dice::ModuleExtensionResult;
pub use extension_execution_dice::compute_bzl_transitive_digest;
pub use extension_execution_dice::compute_bzl_transitive_digest_from_file_states;
pub use extension_execution_dice::extension_spokes_identity_for_workspace;
pub use extension_execution_dice::extract_extension_name;
pub use extension_execution_dice::extract_owning_module;
pub use extension_execution_dice::repo_mapping_overrides_identity_digest;
pub use extension_execution_dice::repo_mappings_identity_digest;
pub use extensions::AggregatedExtension;
pub use extensions::ExtensionResult;
pub use extensions::GeneratedRepo;
pub use extensions::ModuleInfo;
pub use extensions::aggregate_extensions;
pub use extensions::aggregate_extensions_with_policy;
pub use extensions::canonical_extension_id;
pub use extensions::compute_extension_input_hash;
pub use fetch::OverridePatchInput;
pub use fetch::OverridePatchInputs;
pub use fetch::SourceFetcher;
pub use integrity::verify_integrity;
pub use lockfile::Lockfile;
pub use lockfile::LockfileMode;
pub use lockfile::RecordedDirtreeEntryState;
pub use lockfile::SelectedExtensionCache;
pub use lockfile::compute_sha256_hex;
pub use lockfile::compute_sri_hash;
pub use lockfile::lockfile_canonical_extension_id;
pub use lockfile::lockfile_path;
pub use lockfile::parse_lockfile_content;
pub use lockfile::recorded_dirents_input;
pub use lockfile::recorded_dirents_marker_value_from_entries;
pub use lockfile::recorded_dirtree_input;
pub use lockfile::recorded_dirtree_marker_value_from_entry_states;
pub use lockfile::recorded_env_input;
pub use lockfile::recorded_file_input;
pub use lockfile::recorded_file_input_with_recorded_path;
pub use lockfile::recorded_repo_mapping_input;
pub use module_extension_executor::ExtensionExecutionOutput;
pub use module_extension_executor::MODULE_EXTENSION_EXECUTOR_IMPL;
pub use module_extension_executor::ModuleExtensionExecutorImpl;
pub use module_extension_executor::ModuleExtensionMetadata;
pub use parser::ModuleFileParseSession;
pub use parser::ModuleParseError;
pub use parser::ParsedModuleFileWithInputs;
pub use parser::include_label_to_path;
pub use parser::parse_module_bazel;
pub use parser::parse_module_bazel_allow_ignored_extension_repo_directives;
pub use parser::parse_module_bazel_content;
pub use parser::parse_non_root_module_bazel;
pub use parser::parse_non_root_module_bazel_content;
pub use parser::validate_parsed_root_extension_repo_directives;
pub use pending_repo_cells::ExtensionCellDefinitions;
pub use pending_repo_cells::PendingRepoCell;
pub use pending_repo_cells::RepoAlias;
pub use pending_repo_cells::build_all_extension_cells;
pub use pending_repo_cells::build_extension_cell_definitions;
pub use pending_repo_cells::build_extension_cells;
pub use pending_repo_cells::build_use_repo_aliases;
pub use pending_repo_cells::extract_use_repos_for_extension;
pub use pending_repo_cells::is_extension_repo_canonical_name;
pub use pending_repo_cells::parse_canonical_name;
pub use pending_repo_cells::pre_compute_extension_repo_cells;
pub use registry::DEFAULT_REGISTRY_URL;
// `RegisteredToolchain` is defined below; re-export under the crate root for
// consumers that already do `use slug_bzlmod::RegisteredToolchain`.
pub use registry::RegistryClient;
pub use repo_mapping::BzlmodRepoMapping;
pub use repo_mapping::CanonicalLabel;
pub use repo_mapping::CanonicalRepoName;
pub use repo_mapping::ExtensionImportCanonicalization;
pub use repo_mapping::add_extension_generated_repo_mappings;
pub use repo_mapping::canonical_repo_for_extension_import;
pub use repo_mapping::canonicalize_label_with_package_context;
pub use repo_mapping::canonicalize_label_with_package_context_and_repo_resolver;
pub use repo_spec::RepoSpec;
pub use repo_spec::in_extension_context;
pub use repo_spec::record_repo_spec;
pub use repo_spec::with_repo_spec_registry;
pub use repository_execution::ExtensionRepoExecutionKey;
pub use repository_execution::REPOSITORY_MATERIALIZATION_STATE_READER_IMPL;
pub use repository_execution::RepositoryMaterializationStateReader;
pub use repository_execution::RepositoryRuleResult;
pub use repository_execution::repo_execution_spec_hash;
pub use repository_executor::repository_output_digest;
pub use repository_invocations::AttrValue as RepoAttrValue;
pub use repository_invocations::RepositoryInvocation;
pub use resolution::AllowedYankedVersions;
pub use resolution::ModuleKey;
pub use resolution::ModuleSource;
pub use resolution::MvsResolver;
pub use resolution::RemoteModuleResolver;
pub use resolution::ResolvedGraph;
pub use resolution::ResolvedLocalModule;
pub use resolution::ResolvedModuleInfo;
pub use resolution::ResolvedRemoteModule;
pub use resolution::ResolvedRemoteModules;
pub use resolution::parse_allowed_yanked_versions;
pub use resolution::resolve_with_lockfile;
pub use spoke_materialization::materialize_spoke_sync;
pub use spoke_materialization::with_extension_dice;
pub use starlark_repo_rule_executor::STARLARK_REPO_RULE_EXECUTOR_IMPL;
pub use starlark_repo_rule_executor::StarlarkRepoRuleExecution;
pub use starlark_repo_rule_executor::StarlarkRepoRuleExecutorImpl;
pub use starlark_repo_rule_executor::is_builtin_repo_rule;
pub use types::BazelDep;
pub use types::ExtensionTag;
pub use types::ExtensionUsage;
pub use types::Module;
pub use types::Override;
pub use types::ParsedModuleFile;
pub use types::RegisteredItem;
pub use types::TagValue;
pub use types::UseRepo;
pub use version::Version;

pub type RepoMappingSnapshot = BTreeMap<String, BTreeMap<String, String>>;

/// Root-module `override_repo()` rows, keyed by extension id then generated
/// repo internal name.
pub type RepoMappingOverrides = BTreeMap<String, BTreeMap<String, String>>;

/// A registered toolchain entry, tracking its origin module so Plan 13
/// Phase 3's lazy fallback can filter the deferred pool by relevance.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RegisteredToolchain {
    /// Origin module name (root module is marked `is_root = true`).
    pub module: String,
    /// The label string passed to `register_toolchains()`.
    pub label: String,
    /// True iff this registration came from the root module.
    pub is_root: bool,
}

pub trait SetBzlmodDiceInputs {
    fn set_empty_bzlmod_dice_inputs_for_workspace(
        &mut self,
        workspace_id: WorkspaceId,
    ) -> slug_error::Result<()> {
        self.set_bzlmod_cell_graph_data_with_inputs(
            BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone()),
            BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                workspace_id.clone(),
                String::new(),
                Arc::new(HashMap::new()),
            ),
            BzlmodLockfileInputsDataValue::for_workspace_policy(
                workspace_id.clone(),
                LockfileMode::Update,
                None,
                false,
            ),
            BzlmodRepoEnvDataValue::for_workspace(workspace_id.clone(), Arc::new(BTreeMap::new())),
            RegisteredToolchainsDataValue::for_workspace(workspace_id.clone(), Vec::new()),
            RegisteredExecutionPlatformsDataValue::for_workspace(workspace_id.clone(), Vec::new()),
            BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                workspace_id.clone(),
                String::new(),
                Arc::new(HashMap::new()),
            ),
            BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            ),
            BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id,
                Arc::new(RepoMappingSnapshot::new()),
                Arc::new(RepoMappingOverrides::new()),
            ),
        )
    }

    fn set_bzlmod_cell_graph_data_with_inputs(
        &mut self,
        cell_graph: BzlmodCellGraphValue,
        module_versions: BzlmodModuleVersionsDataValue,
        lockfile_inputs: BzlmodLockfileInputsDataValue,
        repo_env: BzlmodRepoEnvDataValue,
        registered_toolchains: RegisteredToolchainsDataValue,
        registered_execution_platforms: RegisteredExecutionPlatformsDataValue,
        extension_aggregations: BzlmodExtensionAggregationsDataValue,
        resolution_facts: BzlmodResolutionFactsValue,
        repo_mappings: BzlmodRepoMappingsDataValue,
    ) -> slug_error::Result<()> {
        self.set_bzlmod_cell_graph_data_with_inputs_digest_and_resolved_graph(
            Arc::from(dice_graph::INJECTED_BZLMOD_PROJECTION_DIGEST),
            cell_graph,
            None,
            module_versions,
            lockfile_inputs,
            repo_env,
            registered_toolchains,
            registered_execution_platforms,
            extension_aggregations,
            resolution_facts,
            repo_mappings,
        )
    }

    fn set_bzlmod_cell_graph_data_with_inputs_digest_and_resolved_graph(
        &mut self,
        cell_graph_resolution_digest: Arc<str>,
        cell_graph: BzlmodCellGraphValue,
        resolved_graph: Option<Arc<ResolvedGraph>>,
        module_versions: BzlmodModuleVersionsDataValue,
        lockfile_inputs: BzlmodLockfileInputsDataValue,
        repo_env: BzlmodRepoEnvDataValue,
        registered_toolchains: RegisteredToolchainsDataValue,
        registered_execution_platforms: RegisteredExecutionPlatformsDataValue,
        extension_aggregations: BzlmodExtensionAggregationsDataValue,
        resolution_facts: BzlmodResolutionFactsValue,
        repo_mappings: BzlmodRepoMappingsDataValue,
    ) -> slug_error::Result<()>;
}

fn validate_cell_graph_workspace(
    field: &str,
    cell_graph_workspace_id: &WorkspaceId,
    data_workspace_id: &WorkspaceId,
) -> slug_error::Result<()> {
    if data_workspace_id != cell_graph_workspace_id {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "BzlmodCellGraphValue carries {} for project root '{}', \
             but its cell graph root is '{}'",
            field,
            data_workspace_id.canonical_project_root.display(),
            cell_graph_workspace_id.canonical_project_root.display()
        ));
    }
    Ok(())
}

fn validate_cell_graph_root_module_name(
    field: &str,
    cell_graph_root_module_name: &str,
    data_root_module_name: &str,
) -> slug_error::Result<()> {
    if data_root_module_name != cell_graph_root_module_name {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "BzlmodCellGraphValue carries {} root module name '{}', \
             but its cell graph root module name is '{}'",
            field,
            data_root_module_name,
            cell_graph_root_module_name
        ));
    }
    Ok(())
}

impl SetBzlmodDiceInputs for dice::DiceTransactionUpdater {
    fn set_bzlmod_cell_graph_data_with_inputs(
        &mut self,
        cell_graph: BzlmodCellGraphValue,
        module_versions: BzlmodModuleVersionsDataValue,
        lockfile_inputs: BzlmodLockfileInputsDataValue,
        repo_env: BzlmodRepoEnvDataValue,
        registered_toolchains: RegisteredToolchainsDataValue,
        registered_execution_platforms: RegisteredExecutionPlatformsDataValue,
        extension_aggregations: BzlmodExtensionAggregationsDataValue,
        resolution_facts: BzlmodResolutionFactsValue,
        repo_mappings: BzlmodRepoMappingsDataValue,
    ) -> slug_error::Result<()> {
        self.set_bzlmod_cell_graph_data_with_inputs_digest_and_resolved_graph(
            Arc::from(dice_graph::INJECTED_BZLMOD_PROJECTION_DIGEST),
            cell_graph,
            None,
            module_versions,
            lockfile_inputs,
            repo_env,
            registered_toolchains,
            registered_execution_platforms,
            extension_aggregations,
            resolution_facts,
            repo_mappings,
        )
    }

    fn set_bzlmod_cell_graph_data_with_inputs_digest_and_resolved_graph(
        &mut self,
        cell_graph_resolution_digest: Arc<str>,
        cell_graph: BzlmodCellGraphValue,
        resolved_graph: Option<Arc<ResolvedGraph>>,
        module_versions: BzlmodModuleVersionsDataValue,
        lockfile_inputs: BzlmodLockfileInputsDataValue,
        repo_env: BzlmodRepoEnvDataValue,
        registered_toolchains: RegisteredToolchainsDataValue,
        registered_execution_platforms: RegisteredExecutionPlatformsDataValue,
        extension_aggregations: BzlmodExtensionAggregationsDataValue,
        resolution_facts: BzlmodResolutionFactsValue,
        repo_mappings: BzlmodRepoMappingsDataValue,
    ) -> slug_error::Result<()> {
        let cell_graph_workspace_id = &cell_graph.workspace_id;
        validate_cell_graph_workspace(
            "module-version data",
            cell_graph_workspace_id,
            &module_versions.workspace_id,
        )?;
        validate_cell_graph_root_module_name(
            "module-version data",
            &cell_graph.root_module_name,
            &module_versions.root_module_name,
        )?;
        validate_cell_graph_workspace(
            "registered-toolchain data",
            cell_graph_workspace_id,
            &registered_toolchains.workspace_id,
        )?;
        validate_cell_graph_workspace(
            "registered-execution-platform data",
            cell_graph_workspace_id,
            &registered_execution_platforms.workspace_id,
        )?;
        validate_cell_graph_workspace(
            "extension-aggregation data",
            cell_graph_workspace_id,
            &extension_aggregations.workspace_id,
        )?;
        validate_cell_graph_root_module_name(
            "extension-aggregation data",
            &cell_graph.root_module_name,
            &extension_aggregations.root_module_name,
        )?;
        validate_cell_graph_workspace(
            "lockfile-input data",
            cell_graph_workspace_id,
            &lockfile_inputs.workspace_id,
        )?;
        validate_cell_graph_workspace(
            "repo-env data",
            cell_graph_workspace_id,
            &repo_env.workspace_id,
        )?;
        validate_cell_graph_workspace(
            "resolution-facts data",
            cell_graph_workspace_id,
            &resolution_facts.workspace_id,
        )?;
        validate_cell_graph_workspace(
            "repo-mapping data",
            cell_graph_workspace_id,
            &repo_mappings.workspace_id,
        )?;

        let lockfile_inputs_data = Arc::new(lockfile_inputs);
        let repo_env_data = Arc::new(repo_env);
        let module_versions = Arc::new(module_versions);
        let resolution_facts = Arc::new(resolution_facts);
        let repo_mappings = Arc::new(repo_mappings);
        let extension_aggregations = Arc::new(extension_aggregations);
        let cell_graph_workspace_id = cell_graph_workspace_id.clone();
        let fallback_cell_graph =
            if resolved_graph.is_some() && MODULE_EXTENSION_EXECUTOR_IMPL.get().is_ok() {
                None
            } else {
                Some(Arc::new(cell_graph))
            };
        let cell_graph_data = fallback_cell_graph.clone().map(|fallback_cell_graph| {
            Arc::new(
                BzlmodCellGraphDataValue::for_workspace_with_resolved_graph_and_fallback(
                    cell_graph_workspace_id.clone(),
                    cell_graph_resolution_digest.clone(),
                    Some(fallback_cell_graph),
                ),
            )
        });
        let module_sources = Arc::new(BzlmodModuleSourcesDataValue::for_workspace(
            cell_graph_workspace_id.clone(),
            cell_graph_resolution_digest,
            Arc::new(
                resolved_graph
                    .as_deref()
                    .map(resolved_module_sources_from_graph)
                    .unwrap_or_default(),
            ),
        ));
        let registered_toolchains = Arc::new(registered_toolchains);
        let registered_execution_platforms = Arc::new(registered_execution_platforms);
        self.changed_to(vec![(BzlmodModuleVersionsDataKey, module_versions)])?;
        self.changed_to(vec![(
            BzlmodRegisteredToolchainsDataKey,
            registered_toolchains,
        )])?;
        self.changed_to(vec![(
            BzlmodRegisteredExecutionPlatformsDataKey,
            registered_execution_platforms,
        )])?;
        self.changed_to(vec![(BzlmodRepoMappingsDataKey, repo_mappings)])?;
        self.changed_to(vec![(BzlmodLockfileInputsDataKey, lockfile_inputs_data)])?;
        self.changed_to(vec![(BzlmodRepoEnvDataKey, repo_env_data)])?;
        self.changed_to(vec![(BzlmodResolutionFactsDataKey, resolution_facts)])?;
        self.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            extension_aggregations,
        )])?;
        if let Some(cell_graph_data) = cell_graph_data {
            self.changed_to(vec![(BzlmodCellGraphDataKey, cell_graph_data)])?;
        }
        self.changed_to(vec![(BzlmodModuleSourcesDataKey, module_sources)])?;
        Ok(())
    }
}

pub async fn module_versions_for_current_workspace(
    ctx: &mut dice::DiceComputations<'_>,
) -> slug_error::Result<Arc<ModuleVersionsValue>> {
    let data = ctx.compute(&BzlmodModuleVersionsDataKey).await?;
    let key = ModuleVersionsKey::for_workspace_id(data.workspace_id.clone());
    ctx.compute(&key).await?
}

pub async fn registered_toolchains_for_current_workspace(
    ctx: &mut dice::DiceComputations<'_>,
) -> slug_error::Result<Arc<RegisteredToolchainsValue>> {
    let data = ctx.compute(&BzlmodRegisteredToolchainsDataKey).await?;
    let key = RegisteredToolchainsKey::for_workspace_id(data.workspace_id.clone());
    ctx.compute(&key).await?
}

pub async fn registered_execution_platforms_for_current_workspace(
    ctx: &mut dice::DiceComputations<'_>,
) -> slug_error::Result<Arc<RegisteredExecutionPlatformsValue>> {
    let data = ctx
        .compute(&BzlmodRegisteredExecutionPlatformsDataKey)
        .await?;
    let key = RegisteredExecutionPlatformsKey::for_workspace_id(data.workspace_id.clone());
    ctx.compute(&key).await?
}

pub async fn bzlmod_cell_graph_for_current_workspace(
    ctx: &mut dice::DiceComputations<'_>,
) -> slug_error::Result<Arc<BzlmodCellGraphValue>> {
    let current = ctx.compute(&BzlmodCurrentCellGraphKey).await??;
    bzlmod_cell_graph_for_workspace_id(ctx, current.workspace_id.clone()).await
}

pub async fn bzlmod_resolution_digest_for_current_workspace(
    ctx: &mut dice::DiceComputations<'_>,
) -> slug_error::Result<Arc<str>> {
    let current = ctx.compute(&BzlmodCurrentCellGraphKey).await??;
    Ok(current.resolution_digest.clone())
}

pub async fn bzlmod_cell_graph_for_workspace_id(
    ctx: &mut dice::DiceComputations<'_>,
    workspace_id: WorkspaceId,
) -> slug_error::Result<Arc<BzlmodCellGraphValue>> {
    let current = ctx.compute(&BzlmodCurrentCellGraphKey).await??;
    if current.workspace_id != workspace_id {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "bzlmod cell graph requested for project root '{}', \
             but current bzlmod cell graph root is '{}'",
            workspace_id.canonical_project_root.display(),
            current.workspace_id.canonical_project_root.display()
        ));
    }
    let key = BzlmodCellGraphKey {
        workspace_id,
        resolution_digest: current.resolution_digest.clone(),
    };
    ctx.compute(&key).await?
}

pub async fn bzlmod_workspace_id_for_current_workspace(
    ctx: &mut dice::DiceComputations<'_>,
) -> slug_error::Result<WorkspaceId> {
    let data = ctx.compute(&BzlmodRepoEnvDataKey).await?;
    Ok(data.workspace_id.clone())
}

pub async fn validate_lockfile_extension_replay_for_current_workspace(
    ctx: &mut dice::DiceComputations<'_>,
) -> slug_error::Result<()> {
    let workspace_id = bzlmod_workspace_id_for_current_workspace(ctx).await?;
    let lockfile_inputs = ctx
        .compute(&BzlmodLockfileInputsKey::for_workspace_id(
            workspace_id.clone(),
        ))
        .await??;
    if lockfile_inputs.lockfile_mode == LockfileMode::Off
        || !lockfile_inputs_has_extension_caches(lockfile_inputs.as_ref())
    {
        return Ok(());
    }

    let aggregations = ctx.compute(&BzlmodExtensionAggregationsDataKey).await?;
    if aggregations.workspace_id != workspace_id {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "lockfile extension replay validation requested for project root '{}', \
             but current bzlmod extension aggregation data root is '{}'",
            workspace_id.canonical_project_root.display(),
            aggregations.workspace_id.canonical_project_root.display()
        ));
    }

    let mut extension_ids: Vec<_> = aggregations
        .extension_aggregations
        .keys()
        .filter(|extension_id| {
            lockfile_inputs_has_extension_cache_for(lockfile_inputs.as_ref(), extension_id)
        })
        .cloned()
        .collect();
    extension_ids.sort();

    for extension_id in extension_ids {
        validate_lockfile_extension_replay_for_extension(
            ctx,
            &workspace_id,
            &extension_id,
            lockfile_inputs.as_ref(),
        )
        .await?;
    }

    Ok(())
}

async fn validate_lockfile_extension_replay_for_extension(
    ctx: &mut dice::DiceComputations<'_>,
    workspace_id: &WorkspaceId,
    extension_id: &str,
    lockfile_inputs: &BzlmodLockfileInputsValue,
) -> slug_error::Result<()> {
    let aggregation = ctx
        .compute(&BzlmodExtensionAggregationKey {
            workspace_id: workspace_id.clone(),
            extension_id: Arc::from(extension_id),
        })
        .await??;
    let Some(aggregation) = aggregation else {
        return Ok(());
    };
    let bzl_transitive_digest = ctx
        .compute(&ExtensionBzlTransitiveDigestKey {
            workspace_id: workspace_id.clone(),
            extension_id: Arc::from(extension_id),
            allow_missing_loads: true,
        })
        .await??;
    let usages_digest = compute_extension_input_hash(aggregation.aggregated.as_ref());
    let repo_env = ctx
        .compute(&BzlmodRepoEnvKey::for_workspace_id(workspace_id.clone()))
        .await??;
    let repo_mappings = ctx
        .compute(&BzlmodRepoMappingsKey::for_workspace_id(
            workspace_id.clone(),
        ))
        .await??;

    for lockfile_value in [
        lockfile_inputs.visible_lockfile.as_ref(),
        lockfile_inputs.hidden_lockfile.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        let Some(lockfile) = lockfile_value.lockfile.as_ref() else {
            continue;
        };
        let Some(selected_cache) = lockfile.select_extension_cache_for_workspace(
            extension_id,
            bzl_transitive_digest.digest(),
            &usages_digest,
            Some(workspace_id.canonical_project_root.as_ref()),
            Some(repo_env.as_ref()),
            Some(repo_mappings.repo_mappings.as_ref()),
            Some(aggregation.root_module_name.as_ref()),
            Some(repo_mappings.repo_mapping_overrides.as_ref()),
        ) else {
            continue;
        };
        if extension_execution_dice::selected_cache_recorded_inputs_current(
            ctx,
            workspace_id.clone(),
            extension_id,
            &selected_cache,
        )
        .await?
        {
            selected_cache.record_hit(extension_id);
            return Ok(());
        }
    }

    Ok(())
}

fn lockfile_inputs_has_extension_caches(inputs: &BzlmodLockfileInputsValue) -> bool {
    inputs
        .visible_lockfile
        .as_ref()
        .and_then(|value| value.lockfile.as_ref())
        .is_some_and(|lockfile| lockfile.has_extension_cache())
        || inputs
            .hidden_lockfile
            .as_ref()
            .and_then(|value| value.lockfile.as_ref())
            .is_some_and(|lockfile| lockfile.has_extension_cache())
}

fn lockfile_inputs_has_extension_cache_for(
    inputs: &BzlmodLockfileInputsValue,
    extension_id: &str,
) -> bool {
    inputs
        .visible_lockfile
        .as_ref()
        .and_then(|value| value.lockfile.as_ref())
        .is_some_and(|lockfile| lockfile.has_extension_cache_candidate(extension_id))
        || inputs
            .hidden_lockfile
            .as_ref()
            .and_then(|value| value.lockfile.as_ref())
            .is_some_and(|lockfile| lockfile.has_extension_cache_candidate(extension_id))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn empty_module_versions(workspace_id: WorkspaceId) -> BzlmodModuleVersionsDataValue {
        BzlmodModuleVersionsDataValue::for_workspace(workspace_id, Arc::new(HashMap::new()))
    }

    fn empty_registered_toolchains(workspace_id: WorkspaceId) -> RegisteredToolchainsDataValue {
        RegisteredToolchainsDataValue::for_workspace(workspace_id, Vec::new())
    }

    fn empty_registered_execution_platforms(
        workspace_id: WorkspaceId,
    ) -> RegisteredExecutionPlatformsDataValue {
        RegisteredExecutionPlatformsDataValue::for_workspace(workspace_id, Vec::new())
    }

    fn empty_extension_aggregations(
        workspace_id: WorkspaceId,
    ) -> BzlmodExtensionAggregationsDataValue {
        empty_extension_aggregations_with_root(workspace_id, "")
    }

    fn empty_extension_aggregations_with_root(
        workspace_id: WorkspaceId,
        root_module_name: &str,
    ) -> BzlmodExtensionAggregationsDataValue {
        BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
            workspace_id,
            root_module_name.to_owned(),
            Arc::new(HashMap::new()),
        )
    }

    #[tokio::test]
    async fn set_bzlmod_cell_graph_data_with_inputs_uses_projection_workspace_id()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-projection-workspace"),
            PathBuf::from("/tmp/slug-plan61-custom-output-base"),
        );
        let data = BzlmodCellGraphValue {
            workspace_id: workspace_id.clone(),
            root_module_name: "root_mod".to_owned(),
            cells: Arc::new(vec![BzlmodCellGraphCell {
                name: "root_mod".to_owned(),
                path: String::new(),
                module_setup: None,
                bundled: false,
            }]),
            extension_cells: Arc::new(Vec::new()),
            root_aliases: Arc::new(vec![BzlmodCellGraphAlias {
                apparent_name: "dep".to_owned(),
                target_name: "dep+".to_owned(),
            }]),
            module_symlinks: Arc::new(Vec::new()),
            scoped_aliases: Arc::new(Vec::new()),
            dynamic_aliases: Arc::new(Vec::new()),
        };
        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.set_bzlmod_cell_graph_data_with_inputs(
            data,
            BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                workspace_id.clone(),
                "root_mod".to_owned(),
                Arc::new(HashMap::new()),
            ),
            BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            ),
            BzlmodRepoEnvDataValue::for_workspace(workspace_id.clone(), Arc::new(BTreeMap::new())),
            empty_registered_toolchains(workspace_id.clone()),
            empty_registered_execution_platforms(workspace_id.clone()),
            empty_extension_aggregations_with_root(workspace_id.clone(), "root_mod"),
            BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            ),
            BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(RepoMappingSnapshot::from([(
                    String::new(),
                    BTreeMap::from([("dep".to_owned(), "dep+".to_owned())]),
                )])),
                Arc::new(RepoMappingOverrides::new()),
            ),
        )?;
        let mut dice = updater.commit().await;

        let repo_mappings = dice
            .compute(&BzlmodRepoMappingsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        assert_eq!(
            repo_mappings
                .repo_mappings
                .get("")
                .and_then(|mapping| mapping.get("dep"))
                .map(String::as_str),
            Some("dep+")
        );
        let cell_graph = dice
            .compute(&BzlmodCellGraphKey::for_workspace_id(workspace_id.clone()))
            .await??;
        assert_eq!(cell_graph.workspace_id, workspace_id);
        assert_eq!(cell_graph.root_module_name, "root_mod");
        assert_eq!(cell_graph.cells[0].name, "root_mod");
        assert_eq!(cell_graph.root_aliases[0].apparent_name, "dep");
        let module_versions = dice
            .compute(&ModuleVersionsKey::for_workspace_id(workspace_id.clone()))
            .await??;
        assert_eq!(module_versions.invalidation.root_module_name, "root_mod");
        let extension_aggregations = dice.compute(&BzlmodExtensionAggregationsDataKey).await?;
        assert_eq!(extension_aggregations.workspace_id, workspace_id);
        assert_eq!(extension_aggregations.root_module_name, "root_mod");

        Ok(())
    }

    #[tokio::test]
    async fn set_bzlmod_cell_graph_data_rejects_mismatched_workspace_provenance()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-projection-consistency-workspace"),
            PathBuf::from("/tmp/slug-plan61-projection-consistency-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-projection-consistency-other"),
            PathBuf::from("/tmp/slug-plan61-projection-consistency-other-output"),
        );
        let data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        let lockfile_inputs = BzlmodLockfileInputsDataValue::for_workspace(
            workspace_id.clone(),
            Arc::new(BzlmodLockfileInputsValue::default()),
        );
        let repo_env =
            BzlmodRepoEnvDataValue::for_workspace(other_workspace_id, Arc::new(BTreeMap::new()));
        let resolution_facts = BzlmodResolutionFactsValue::for_workspace(
            workspace_id.clone(),
            indexmap::IndexMap::new(),
            indexmap::IndexMap::new(),
        );
        let repo_mappings = BzlmodRepoMappingsDataValue::for_workspace(
            workspace_id.clone(),
            Arc::new(RepoMappingSnapshot::new()),
            Arc::new(RepoMappingOverrides::new()),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        let err = updater
            .set_bzlmod_cell_graph_data_with_inputs(
                data,
                empty_module_versions(workspace_id.clone()),
                lockfile_inputs,
                repo_env,
                empty_registered_toolchains(workspace_id.clone()),
                empty_registered_execution_platforms(workspace_id.clone()),
                empty_extension_aggregations(workspace_id.clone()),
                resolution_facts,
                repo_mappings,
            )
            .unwrap_err();
        assert!(err.to_string().contains("repo-env data"), "{err:?}");
        assert!(
            err.to_string().contains(
                "but its cell graph root is '/tmp/slug-plan61-projection-consistency-workspace'"
            ),
            "{err:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn set_bzlmod_cell_graph_data_rejects_mismatched_module_projection_provenance()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-module-provenance-workspace"),
            PathBuf::from("/tmp/slug-plan61-module-provenance-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-module-provenance-other"),
            PathBuf::from("/tmp/slug-plan61-module-provenance-other-output"),
        );
        let data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        let err = updater
            .set_bzlmod_cell_graph_data_with_inputs(
                data,
                empty_module_versions(other_workspace_id),
                BzlmodLockfileInputsDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BzlmodLockfileInputsValue::default()),
                ),
                BzlmodRepoEnvDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BTreeMap::new()),
                ),
                empty_registered_toolchains(workspace_id.clone()),
                empty_registered_execution_platforms(workspace_id.clone()),
                empty_extension_aggregations(workspace_id.clone()),
                BzlmodResolutionFactsValue::for_workspace(
                    workspace_id.clone(),
                    indexmap::IndexMap::new(),
                    indexmap::IndexMap::new(),
                ),
                BzlmodRepoMappingsDataValue::for_workspace(
                    workspace_id,
                    Arc::new(RepoMappingSnapshot::new()),
                    Arc::new(RepoMappingOverrides::new()),
                ),
            )
            .unwrap_err();
        assert!(err.to_string().contains("module-version data"), "{err:?}");

        Ok(())
    }

    #[tokio::test]
    async fn set_bzlmod_cell_graph_data_rejects_mismatched_lockfile_input_provenance()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-lockfile-provenance-workspace"),
            PathBuf::from("/tmp/slug-plan61-lockfile-provenance-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-lockfile-provenance-other"),
            PathBuf::from("/tmp/slug-plan61-lockfile-provenance-other-output"),
        );
        let data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        let repo_env = BzlmodRepoEnvDataValue::for_workspace(
            data.workspace_id.clone(),
            Arc::new(BTreeMap::new()),
        );
        let resolution_facts = BzlmodResolutionFactsValue::for_workspace(
            data.workspace_id.clone(),
            indexmap::IndexMap::new(),
            indexmap::IndexMap::new(),
        );
        let repo_mappings = BzlmodRepoMappingsDataValue::for_workspace(
            data.workspace_id.clone(),
            Arc::new(RepoMappingSnapshot::new()),
            Arc::new(RepoMappingOverrides::new()),
        );
        let lockfile_inputs = BzlmodLockfileInputsDataValue::for_workspace(
            other_workspace_id,
            Arc::new(BzlmodLockfileInputsValue::default()),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        let err = updater
            .set_bzlmod_cell_graph_data_with_inputs(
                data,
                empty_module_versions(workspace_id.clone()),
                lockfile_inputs,
                repo_env,
                empty_registered_toolchains(workspace_id.clone()),
                empty_registered_execution_platforms(workspace_id.clone()),
                empty_extension_aggregations(workspace_id.clone()),
                resolution_facts,
                repo_mappings,
            )
            .unwrap_err();
        assert!(err.to_string().contains("lockfile-input data"), "{err:?}");
        assert!(
            err.to_string().contains(
                "but its cell graph root is '/tmp/slug-plan61-lockfile-provenance-workspace'"
            ),
            "{err:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn set_bzlmod_cell_graph_data_rejects_mismatched_registration_projection_provenance()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-registration-provenance-workspace"),
            PathBuf::from("/tmp/slug-plan61-registration-provenance-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-registration-provenance-other"),
            PathBuf::from("/tmp/slug-plan61-registration-provenance-other-output"),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        let data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        let err = updater
            .set_bzlmod_cell_graph_data_with_inputs(
                data,
                empty_module_versions(workspace_id.clone()),
                BzlmodLockfileInputsDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BzlmodLockfileInputsValue::default()),
                ),
                BzlmodRepoEnvDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BTreeMap::new()),
                ),
                empty_registered_toolchains(other_workspace_id.clone()),
                empty_registered_execution_platforms(workspace_id.clone()),
                empty_extension_aggregations(workspace_id.clone()),
                BzlmodResolutionFactsValue::for_workspace(
                    workspace_id.clone(),
                    indexmap::IndexMap::new(),
                    indexmap::IndexMap::new(),
                ),
                BzlmodRepoMappingsDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(RepoMappingSnapshot::new()),
                    Arc::new(RepoMappingOverrides::new()),
                ),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("registered-toolchain data"),
            "{err:?}"
        );

        let data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        let err = updater
            .set_bzlmod_cell_graph_data_with_inputs(
                data,
                empty_module_versions(workspace_id.clone()),
                BzlmodLockfileInputsDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BzlmodLockfileInputsValue::default()),
                ),
                BzlmodRepoEnvDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BTreeMap::new()),
                ),
                empty_registered_toolchains(workspace_id.clone()),
                empty_registered_execution_platforms(other_workspace_id),
                empty_extension_aggregations(workspace_id.clone()),
                BzlmodResolutionFactsValue::for_workspace(
                    workspace_id.clone(),
                    indexmap::IndexMap::new(),
                    indexmap::IndexMap::new(),
                ),
                BzlmodRepoMappingsDataValue::for_workspace(
                    workspace_id,
                    Arc::new(RepoMappingSnapshot::new()),
                    Arc::new(RepoMappingOverrides::new()),
                ),
            )
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("registered-execution-platform data"),
            "{err:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn set_bzlmod_cell_graph_data_rejects_mismatched_extension_projection_provenance()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-extension-provenance-workspace"),
            PathBuf::from("/tmp/slug-plan61-extension-provenance-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-extension-provenance-other"),
            PathBuf::from("/tmp/slug-plan61-extension-provenance-other-output"),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        let data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        let err = updater
            .set_bzlmod_cell_graph_data_with_inputs(
                data,
                empty_module_versions(workspace_id.clone()),
                BzlmodLockfileInputsDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BzlmodLockfileInputsValue::default()),
                ),
                BzlmodRepoEnvDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BTreeMap::new()),
                ),
                empty_registered_toolchains(workspace_id.clone()),
                empty_registered_execution_platforms(workspace_id.clone()),
                empty_extension_aggregations(other_workspace_id),
                BzlmodResolutionFactsValue::for_workspace(
                    workspace_id.clone(),
                    indexmap::IndexMap::new(),
                    indexmap::IndexMap::new(),
                ),
                BzlmodRepoMappingsDataValue::for_workspace(
                    workspace_id,
                    Arc::new(RepoMappingSnapshot::new()),
                    Arc::new(RepoMappingOverrides::new()),
                ),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("extension-aggregation data"),
            "{err:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn set_bzlmod_cell_graph_data_rejects_mismatched_extension_root_name()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-extension-root-provenance-workspace"),
            PathBuf::from("/tmp/slug-plan61-extension-root-provenance-output-base"),
        );
        let mut data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        data.root_module_name = "root_mod".to_owned();

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        let err = updater
            .set_bzlmod_cell_graph_data_with_inputs(
                data,
                BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root_mod".to_owned(),
                    Arc::new(HashMap::new()),
                ),
                BzlmodLockfileInputsDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BzlmodLockfileInputsValue::default()),
                ),
                BzlmodRepoEnvDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BTreeMap::new()),
                ),
                empty_registered_toolchains(workspace_id.clone()),
                empty_registered_execution_platforms(workspace_id.clone()),
                empty_extension_aggregations_with_root(workspace_id.clone(), "stale_root"),
                BzlmodResolutionFactsValue::for_workspace(
                    workspace_id.clone(),
                    indexmap::IndexMap::new(),
                    indexmap::IndexMap::new(),
                ),
                BzlmodRepoMappingsDataValue::for_workspace(
                    workspace_id,
                    Arc::new(RepoMappingSnapshot::new()),
                    Arc::new(RepoMappingOverrides::new()),
                ),
            )
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("extension-aggregation data root module name"),
            "{err:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn set_bzlmod_cell_graph_data_rejects_mismatched_module_root_name()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-module-root-provenance-workspace"),
            PathBuf::from("/tmp/slug-plan61-module-root-provenance-output-base"),
        );
        let mut data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        data.root_module_name = "root_mod".to_owned();

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        let err = updater
            .set_bzlmod_cell_graph_data_with_inputs(
                data,
                BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "stale_root".to_owned(),
                    Arc::new(HashMap::new()),
                ),
                BzlmodLockfileInputsDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BzlmodLockfileInputsValue::default()),
                ),
                BzlmodRepoEnvDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BTreeMap::new()),
                ),
                empty_registered_toolchains(workspace_id.clone()),
                empty_registered_execution_platforms(workspace_id.clone()),
                empty_extension_aggregations_with_root(workspace_id.clone(), "root_mod"),
                BzlmodResolutionFactsValue::for_workspace(
                    workspace_id.clone(),
                    indexmap::IndexMap::new(),
                    indexmap::IndexMap::new(),
                ),
                BzlmodRepoMappingsDataValue::for_workspace(
                    workspace_id,
                    Arc::new(RepoMappingSnapshot::new()),
                    Arc::new(RepoMappingOverrides::new()),
                ),
            )
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("module-version data root module name"),
            "{err:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn set_bzlmod_cell_graph_data_rejects_mismatched_resolution_projection_provenance()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-resolution-provenance-workspace"),
            PathBuf::from("/tmp/slug-plan61-resolution-provenance-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-resolution-provenance-other"),
            PathBuf::from("/tmp/slug-plan61-resolution-provenance-other-output"),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        let data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        let err = updater
            .set_bzlmod_cell_graph_data_with_inputs(
                data,
                empty_module_versions(workspace_id.clone()),
                BzlmodLockfileInputsDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BzlmodLockfileInputsValue::default()),
                ),
                BzlmodRepoEnvDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BTreeMap::new()),
                ),
                empty_registered_toolchains(workspace_id.clone()),
                empty_registered_execution_platforms(workspace_id.clone()),
                empty_extension_aggregations(workspace_id.clone()),
                BzlmodResolutionFactsValue::for_workspace(
                    other_workspace_id.clone(),
                    indexmap::IndexMap::new(),
                    indexmap::IndexMap::new(),
                ),
                BzlmodRepoMappingsDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(RepoMappingSnapshot::new()),
                    Arc::new(RepoMappingOverrides::new()),
                ),
            )
            .unwrap_err();
        assert!(err.to_string().contains("resolution-facts data"), "{err:?}");

        let data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        let err = updater
            .set_bzlmod_cell_graph_data_with_inputs(
                data,
                empty_module_versions(workspace_id.clone()),
                BzlmodLockfileInputsDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BzlmodLockfileInputsValue::default()),
                ),
                BzlmodRepoEnvDataValue::for_workspace(
                    workspace_id.clone(),
                    Arc::new(BTreeMap::new()),
                ),
                empty_registered_toolchains(workspace_id.clone()),
                empty_registered_execution_platforms(workspace_id.clone()),
                empty_extension_aggregations(workspace_id.clone()),
                BzlmodResolutionFactsValue::for_workspace(
                    workspace_id.clone(),
                    indexmap::IndexMap::new(),
                    indexmap::IndexMap::new(),
                ),
                BzlmodRepoMappingsDataValue::for_workspace(
                    other_workspace_id,
                    Arc::new(RepoMappingSnapshot::new()),
                    Arc::new(RepoMappingOverrides::new()),
                ),
            )
            .unwrap_err();
        assert!(err.to_string().contains("repo-mapping data"), "{err:?}");

        Ok(())
    }

    #[tokio::test]
    async fn set_bzlmod_cell_graph_data_injects_separate_lockfile_inputs() -> slug_error::Result<()>
    {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-projection-lockfile-digest"),
            PathBuf::from("/tmp/slug-plan61-projection-lockfile-output-base"),
        );
        let data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        let visible_lockfile = Arc::new(LockfileContentValue {
            path: Arc::new(PathBuf::from(
                "/tmp/slug-plan61-projection-lockfile-digest/MODULE.bazel.lock",
            )),
            digest: Some("visible-digest".to_owned()),
            tracked_by_dice: true,
            lockfile: None,
        });
        let hidden_lockfile = Arc::new(LockfileContentValue {
            path: Arc::new(PathBuf::from(
                "/tmp/slug-plan61-projection-lockfile-output-base/MODULE.bazel.lock",
            )),
            digest: Some("hidden-digest".to_owned()),
            tracked_by_dice: true,
            lockfile: None,
        });
        let lockfile_inputs = BzlmodLockfileInputsDataValue::for_workspace(
            workspace_id.clone(),
            Arc::new(BzlmodLockfileInputsValue::from_values(
                Some(PathBuf::from(
                    "/tmp/slug-plan61-projection-lockfile-output-base/MODULE.bazel.lock",
                )),
                Some(visible_lockfile),
                Some(hidden_lockfile),
                LockfileMode::Update,
            )),
        );
        let repo_env = BzlmodRepoEnvDataValue::for_workspace(
            workspace_id.clone(),
            Arc::new(BTreeMap::from([(
                "TOKEN".to_owned(),
                "from-projection".to_owned(),
            )])),
        );
        let mut resolution_facts = BzlmodResolutionFactsValue::for_workspace(
            workspace_id.clone(),
            indexmap::IndexMap::new(),
            indexmap::IndexMap::new(),
        );
        resolution_facts.registry_file_hashes.insert(
            "registry/modules/dep/1.0/MODULE.bazel".to_owned(),
            "sha256-registry".to_owned(),
        );
        resolution_facts
            .selected_yanked_versions
            .insert("dep@1.0".to_owned(), "allowed by flag".to_owned());
        let repo_mappings = BzlmodRepoMappingsDataValue::for_workspace(
            workspace_id.clone(),
            Arc::new(RepoMappingSnapshot::new()),
            Arc::new(RepoMappingOverrides::new()),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.set_bzlmod_cell_graph_data_with_inputs(
            data,
            empty_module_versions(workspace_id.clone()),
            lockfile_inputs,
            repo_env,
            empty_registered_toolchains(workspace_id.clone()),
            empty_registered_execution_platforms(workspace_id.clone()),
            empty_extension_aggregations(workspace_id.clone()),
            resolution_facts,
            repo_mappings,
        )?;
        let mut dice = updater.commit().await;

        let lockfile_inputs = dice
            .compute(&BzlmodLockfileInputsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        assert_eq!(
            lockfile_inputs.visible_lockfile_digest.as_deref(),
            Some("visible-digest")
        );
        assert_eq!(
            lockfile_inputs.hidden_lockfile_digest.as_deref(),
            Some("hidden-digest")
        );

        let repo_env = dice
            .compute(&BzlmodRepoEnvKey::for_workspace_id(workspace_id.clone()))
            .await??;
        assert_eq!(
            repo_env.get("TOKEN").map(String::as_str),
            Some("from-projection")
        );
        let resolution_facts = dice
            .compute(&BzlmodResolutionFactsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        assert_eq!(
            resolution_facts
                .registry_file_hashes
                .get("registry/modules/dep/1.0/MODULE.bazel")
                .map(String::as_str),
            Some("sha256-registry")
        );
        assert_eq!(
            resolution_facts
                .selected_yanked_versions
                .get("dep@1.0")
                .map(String::as_str),
            Some("allowed by flag")
        );

        let module_versions = dice
            .compute(&ModuleVersionsKey::for_workspace_id(workspace_id.clone()))
            .await??;
        assert_eq!(
            module_versions
                .invalidation
                .repo_env
                .get("TOKEN")
                .map(String::as_str),
            Some("from-projection")
        );
        assert_eq!(
            module_versions
                .invalidation
                .lockfile_inputs
                .visible_lockfile_digest
                .as_deref(),
            Some("visible-digest")
        );
        assert_eq!(
            module_versions
                .invalidation
                .lockfile_inputs
                .hidden_lockfile_digest
                .as_deref(),
            Some("hidden-digest")
        );
        assert_eq!(
            module_versions
                .invalidation
                .registry_file_hashes
                .get("registry/modules/dep/1.0/MODULE.bazel")
                .map(String::as_str),
            Some("sha256-registry")
        );
        assert_eq!(
            module_versions
                .invalidation
                .selected_yanked_versions
                .get("dep@1.0")
                .map(String::as_str),
            Some("allowed by flag")
        );

        Ok(())
    }

    #[tokio::test]
    async fn semantic_cell_graph_keys_use_cell_graph_workspace() -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-registered-cell-graph-workspace"),
            PathBuf::from("/tmp/slug-plan61-registered-cell-graph-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-registered-cell-graph-other"),
            PathBuf::from("/tmp/slug-plan61-registered-cell-graph-other-output"),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodCellGraphDataKey,
            Arc::new(BzlmodCellGraphDataValue::for_workspace(
                workspace_id.clone(),
                Arc::from(dice_graph::INJECTED_BZLMOD_PROJECTION_DIGEST),
                Arc::new(BzlmodCellGraphValue::empty_for_workspace(
                    workspace_id.clone(),
                )),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(BzlmodModuleVersionsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(HashMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoMappingsDataKey,
            Arc::new(BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(RepoMappingSnapshot::new()),
                Arc::new(RepoMappingOverrides::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRegisteredToolchainsDataKey,
            Arc::new(RegisteredToolchainsDataValue::for_workspace(
                workspace_id.clone(),
                Vec::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRegisteredExecutionPlatformsDataKey,
            Arc::new(RegisteredExecutionPlatformsDataValue::for_workspace(
                workspace_id.clone(),
                Vec::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            Arc::new(empty_extension_aggregations(workspace_id.clone())),
        )])?;
        let mut dice = updater.commit().await;

        let module_versions = dice
            .compute(&ModuleVersionsKey::for_workspace_id(workspace_id.clone()))
            .await??;
        let registered_toolchains = dice
            .compute(&RegisteredToolchainsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        let registered_execution_platforms = dice
            .compute(&RegisteredExecutionPlatformsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        let repo_mappings = dice
            .compute(&BzlmodRepoMappingsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        let lockfile_inputs = dice
            .compute(&BzlmodLockfileInputsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        let repo_env = dice
            .compute(&BzlmodRepoEnvKey::for_workspace_id(workspace_id.clone()))
            .await??;
        assert_eq!(module_versions.workspace_id, workspace_id);
        assert_eq!(registered_toolchains.workspace_id, workspace_id);
        assert_eq!(registered_execution_platforms.workspace_id, workspace_id);
        assert_eq!(repo_mappings.workspace_id, workspace_id);
        assert_eq!(lockfile_inputs.lockfile_mode, LockfileMode::Update);
        assert!(repo_env.is_empty());
        let cell_graph = dice
            .compute(&BzlmodCellGraphKey::for_workspace_id(workspace_id.clone()))
            .await??;
        assert_eq!(cell_graph.workspace_id, workspace_id);

        assert!(
            dice.compute(&ModuleVersionsKey::for_workspace_id(
                other_workspace_id.clone(),
            ))
            .await?
            .is_err()
        );
        assert!(
            dice.compute(&BzlmodLockfileInputsKey::for_workspace_id(
                other_workspace_id.clone(),
            ))
            .await?
            .is_err()
        );
        assert!(
            dice.compute(&BzlmodRepoEnvKey::for_workspace_id(
                other_workspace_id.clone(),
            ))
            .await?
            .is_err()
        );
        assert!(
            dice.compute(&BzlmodRepoMappingsKey::for_workspace_id(
                other_workspace_id.clone(),
            ))
            .await?
            .is_err()
        );
        assert!(
            dice.compute(&RegisteredToolchainsKey::for_workspace_id(
                other_workspace_id.clone(),
            ))
            .await?
            .is_err()
        );
        assert!(
            dice.compute(&RegisteredExecutionPlatformsKey::for_workspace_id(
                other_workspace_id.clone(),
            ))
            .await?
            .is_err()
        );

        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            Arc::new(empty_extension_aggregations(other_workspace_id.clone())),
        )])?;
        let mut dice = updater.commit().await;
        let err = dice
            .compute(&BzlmodCellGraphKey::for_workspace_id(workspace_id.clone()))
            .await?
            .unwrap_err();
        assert!(
            err.to_string().contains("extension aggregation data root"),
            "{err:?}"
        );

        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            Arc::new(empty_extension_aggregations(workspace_id.clone())),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoMappingsDataKey,
            Arc::new(BzlmodRepoMappingsDataValue::for_workspace(
                other_workspace_id,
                Arc::new(RepoMappingSnapshot::new()),
                Arc::new(RepoMappingOverrides::new()),
            )),
        )])?;
        let mut dice = updater.commit().await;
        let err = dice
            .compute(&BzlmodRepoMappingsKey::for_workspace_id(workspace_id))
            .await?
            .unwrap_err();
        assert!(
            err.to_string().contains("repo mapping data root"),
            "{err:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn module_versions_key_uses_module_data_root_name_without_cell_graph()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-module-versions-no-cell-graph"),
            PathBuf::from("/tmp/slug-plan61-module-versions-no-cell-graph-output"),
        );
        let module_versions = BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
            workspace_id.clone(),
            "root_from_module_data".to_owned(),
            Arc::new(HashMap::new()),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(module_versions),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoMappingsDataKey,
            Arc::new(BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(RepoMappingSnapshot::new()),
                Arc::new(RepoMappingOverrides::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        let mut dice = updater.commit().await;

        let module_versions = dice
            .compute(&ModuleVersionsKey::for_workspace_id(workspace_id.clone()))
            .await??;

        assert_eq!(module_versions.workspace_id, workspace_id);
        assert_eq!(
            module_versions.invalidation.root_module_name,
            "root_from_module_data"
        );

        Ok(())
    }

    #[tokio::test]
    async fn cell_graph_key_uses_module_data_root_name() -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-cell-graph-module-root"),
            PathBuf::from("/tmp/slug-plan61-cell-graph-module-root-output"),
        );
        let mut graph = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        graph.root_module_name = "root_from_graph_payload".to_owned();

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodCellGraphDataKey,
            Arc::new(BzlmodCellGraphDataValue::for_workspace(
                workspace_id.clone(),
                Arc::from(dice_graph::INJECTED_BZLMOD_PROJECTION_DIGEST),
                Arc::new(graph),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(
                BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root_from_module_data".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoMappingsDataKey,
            Arc::new(BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(RepoMappingSnapshot::new()),
                Arc::new(RepoMappingOverrides::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            Arc::new(
                BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root_from_module_data".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        let mut dice = updater.commit().await;

        let cell_graph = dice
            .compute(&BzlmodCellGraphKey::for_workspace_id(workspace_id))
            .await??;

        assert_eq!(cell_graph.root_module_name, "root_from_module_data");

        Ok(())
    }

    #[tokio::test]
    async fn cell_graph_key_uses_repo_mapping_scoped_aliases() -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-cell-graph-scoped-aliases"),
            PathBuf::from("/tmp/slug-plan61-cell-graph-scoped-aliases-output"),
        );
        let mut graph = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        graph.root_module_name = "root".to_owned();
        graph.scoped_aliases = Arc::new(vec![BzlmodCellGraphScopedAlias {
            owner_module: "payload_owner".to_owned(),
            apparent_name: "payload_alias".to_owned(),
            target_name: "payload_target".to_owned(),
        }]);

        let mut repo_mapping_snapshot = RepoMappingSnapshot::new();
        repo_mapping_snapshot.insert(
            "dep".to_owned(),
            BTreeMap::from([("tool".to_owned(), "root+tool".to_owned())]),
        );
        let mut repo_mapping_overrides = RepoMappingOverrides::new();
        repo_mapping_overrides.insert(
            "@@root//:ext.bzl%ext".to_owned(),
            BTreeMap::from([("override_tool".to_owned(), "dep+".to_owned())]),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodCellGraphDataKey,
            Arc::new(BzlmodCellGraphDataValue::for_workspace(
                workspace_id.clone(),
                Arc::from(dice_graph::INJECTED_BZLMOD_PROJECTION_DIGEST),
                Arc::new(graph),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(
                BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoMappingsDataKey,
            Arc::new(BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(repo_mapping_snapshot),
                Arc::new(repo_mapping_overrides),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            Arc::new(
                BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        let mut dice = updater.commit().await;

        let cell_graph = dice
            .compute(&BzlmodCellGraphKey::for_workspace_id(workspace_id))
            .await??;

        assert_eq!(
            cell_graph.scoped_aliases.as_ref(),
            &vec![
                BzlmodCellGraphScopedAlias {
                    owner_module: "dep".to_owned(),
                    apparent_name: "tool".to_owned(),
                    target_name: "root+tool".to_owned(),
                },
                BzlmodCellGraphScopedAlias {
                    owner_module: "_main".to_owned(),
                    apparent_name: "override_tool".to_owned(),
                    target_name: "dep+".to_owned(),
                },
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn cell_graph_key_uses_root_repo_mapping_aliases() -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-cell-graph-root-aliases"),
            PathBuf::from("/tmp/slug-plan61-cell-graph-root-aliases-output"),
        );
        let mut graph = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        graph.root_module_name = "root".to_owned();
        graph.root_aliases = Arc::new(vec![BzlmodCellGraphAlias {
            apparent_name: "payload_alias".to_owned(),
            target_name: "payload_target".to_owned(),
        }]);

        let mut repo_mapping_snapshot = RepoMappingSnapshot::new();
        repo_mapping_snapshot.insert(
            String::new(),
            BTreeMap::from([
                ("dep".to_owned(), "dep+".to_owned()),
                ("tool".to_owned(), "root+tool".to_owned()),
            ]),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodCellGraphDataKey,
            Arc::new(BzlmodCellGraphDataValue::for_workspace(
                workspace_id.clone(),
                Arc::from(dice_graph::INJECTED_BZLMOD_PROJECTION_DIGEST),
                Arc::new(graph),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(
                BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoMappingsDataKey,
            Arc::new(BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(repo_mapping_snapshot),
                Arc::new(RepoMappingOverrides::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            Arc::new(
                BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        let mut dice = updater.commit().await;

        let cell_graph = dice
            .compute(&BzlmodCellGraphKey::for_workspace_id(workspace_id))
            .await??;

        assert_eq!(
            cell_graph.root_aliases.as_ref(),
            &vec![
                BzlmodCellGraphAlias {
                    apparent_name: "dep".to_owned(),
                    target_name: "dep+".to_owned(),
                },
                BzlmodCellGraphAlias {
                    apparent_name: "tool".to_owned(),
                    target_name: "root+tool".to_owned(),
                },
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn cell_graph_key_uses_root_repo_mapping_dynamic_aliases() -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-cell-graph-dynamic-aliases"),
            PathBuf::from("/tmp/slug-plan61-cell-graph-dynamic-aliases-output"),
        );
        let mut graph = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        graph.root_module_name = "root".to_owned();
        graph.dynamic_aliases = Arc::new(vec![BzlmodCellGraphDynamicAlias {
            apparent_name: "payload_alias".to_owned(),
            canonical_name: "payload_target".to_owned(),
        }]);

        let mut repo_mapping_snapshot = RepoMappingSnapshot::new();
        repo_mapping_snapshot.insert(
            String::new(),
            BTreeMap::from([
                ("_main+ext+generated".to_owned(), "dep+".to_owned()),
                ("ordinary".to_owned(), "dep+".to_owned()),
            ]),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodCellGraphDataKey,
            Arc::new(BzlmodCellGraphDataValue::for_workspace(
                workspace_id.clone(),
                Arc::from(dice_graph::INJECTED_BZLMOD_PROJECTION_DIGEST),
                Arc::new(graph),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(
                BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoMappingsDataKey,
            Arc::new(BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(repo_mapping_snapshot),
                Arc::new(RepoMappingOverrides::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            Arc::new(
                BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        let mut dice = updater.commit().await;

        let cell_graph = dice
            .compute(&BzlmodCellGraphKey::for_workspace_id(workspace_id))
            .await??;

        assert_eq!(
            cell_graph.dynamic_aliases.as_ref(),
            &vec![BzlmodCellGraphDynamicAlias {
                apparent_name: "_main+ext+generated".to_owned(),
                canonical_name: "dep+".to_owned(),
            }]
        );

        Ok(())
    }

    #[tokio::test]
    async fn cell_graph_key_derives_module_symlinks_from_cell_setup() -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-cell-graph-module-symlinks"),
            PathBuf::from("/tmp/slug-plan61-cell-graph-module-symlinks-output"),
        );
        let mut graph = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        graph.root_module_name = "root".to_owned();
        graph.cells = Arc::new(vec![
            BzlmodCellGraphCell {
                name: "dep+".to_owned(),
                path: "bazel-external/dep+".to_owned(),
                module_setup: Some(BzlmodCellGraphModuleSetup {
                    module_name: "dep".to_owned(),
                    version: "1.0".to_owned(),
                    registry_url: "https://registry.example".to_owned(),
                    source_path: "/tmp/slug-plan61-derived-dep".to_owned(),
                }),
                bundled: false,
            },
            BzlmodCellGraphCell {
                name: "local+".to_owned(),
                path: "bazel-external/local+".to_owned(),
                module_setup: None,
                bundled: false,
            },
        ]);
        graph.module_symlinks = Arc::new(vec![
            BzlmodCellGraphModuleSymlink {
                entry_name: "dep+".to_owned(),
                source_path: Arc::new(PathBuf::from("/tmp/slug-plan61-payload-dep")),
            },
            BzlmodCellGraphModuleSymlink {
                entry_name: "local+".to_owned(),
                source_path: Arc::new(PathBuf::from("/tmp/slug-plan61-local-dep")),
            },
        ]);

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodCellGraphDataKey,
            Arc::new(BzlmodCellGraphDataValue::for_workspace(
                workspace_id.clone(),
                Arc::from(dice_graph::INJECTED_BZLMOD_PROJECTION_DIGEST),
                Arc::new(graph),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(
                BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoMappingsDataKey,
            Arc::new(BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(RepoMappingSnapshot::new()),
                Arc::new(RepoMappingOverrides::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            Arc::new(
                BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        let mut dice = updater.commit().await;

        let cell_graph = dice
            .compute(&BzlmodCellGraphKey::for_workspace_id(workspace_id))
            .await??;

        assert_eq!(
            cell_graph.module_symlinks.as_ref(),
            &vec![
                BzlmodCellGraphModuleSymlink {
                    entry_name: "dep+".to_owned(),
                    source_path: Arc::new(PathBuf::from("/tmp/slug-plan61-derived-dep")),
                },
                BzlmodCellGraphModuleSymlink {
                    entry_name: "local+".to_owned(),
                    source_path: Arc::new(PathBuf::from("/tmp/slug-plan61-local-dep")),
                },
            ]
        );

        Ok(())
    }

    #[tokio::test]
    async fn cell_graph_key_derives_module_cells_from_resolved_graph() -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-cell-graph-resolved-modules"),
            PathBuf::from("/tmp/slug-plan61-cell-graph-resolved-modules-output"),
        );
        let mut payload_graph = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());
        payload_graph.root_module_name = "root".to_owned();
        payload_graph.cells = Arc::new(vec![BzlmodCellGraphCell {
            name: "stale+".to_owned(),
            path: "bazel-external/stale+".to_owned(),
            module_setup: None,
            bundled: false,
        }]);

        let mut resolved_graph = ResolvedGraph::default();
        resolved_graph.modules.insert(
            "root".to_owned(),
            ResolvedModuleInfo {
                name: "root".to_owned(),
                version: String::new(),
                compatibility_level: 0,
                dependencies: HashMap::new(),
                source: ModuleSource::LocalPath {
                    path: ".".to_owned(),
                },
                source_path: None,
            },
        );
        resolved_graph.modules.insert(
            "dep".to_owned(),
            ResolvedModuleInfo {
                name: "dep".to_owned(),
                version: "1.0".to_owned(),
                compatibility_level: 0,
                dependencies: HashMap::new(),
                source: ModuleSource::Registry {
                    url: "https://registry.example".to_owned(),
                },
                source_path: Some(PathBuf::from("/tmp/slug-plan61-dep-src")),
            },
        );
        resolved_graph.modules.insert(
            "local".to_owned(),
            ResolvedModuleInfo {
                name: "local".to_owned(),
                version: String::new(),
                compatibility_level: 0,
                dependencies: HashMap::new(),
                source: ModuleSource::LocalPath {
                    path: "/tmp/slug-plan61-local-outside".to_owned(),
                },
                source_path: None,
            },
        );
        let resolved_graph = Arc::new(resolved_graph);
        let resolution_digest: Arc<str> = Arc::from("test-resolved-graph-digest");

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodCellGraphDataKey,
            Arc::new(BzlmodCellGraphDataValue::for_workspace_with_resolved_graph(
                workspace_id.clone(),
                resolution_digest.clone(),
                Arc::new(payload_graph),
                Some(resolved_graph.clone()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodModuleSourcesDataKey,
            Arc::new(BzlmodModuleSourcesDataValue::for_workspace(
                workspace_id.clone(),
                resolution_digest.clone(),
                Arc::new(resolved_module_sources_from_graph(&resolved_graph)),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(
                BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(HashMap::from([("dep".to_owned(), "1.0".to_owned())])),
                ),
            ),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoMappingsDataKey,
            Arc::new(BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(RepoMappingSnapshot::new()),
                Arc::new(RepoMappingOverrides::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodExtensionAggregationsDataKey,
            Arc::new(
                BzlmodExtensionAggregationsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        let mut dice = updater.commit().await;

        let cell_graph = dice
            .compute(&BzlmodCellGraphKey {
                workspace_id,
                resolution_digest,
            })
            .await??;

        assert!(cell_graph.cells.iter().any(|cell| {
            cell.name == "dep+"
                && cell.path == "bazel-external/dep+"
                && cell
                    .module_setup
                    .as_ref()
                    .is_some_and(|setup| setup.source_path == "/tmp/slug-plan61-dep-src")
        }));
        assert!(!cell_graph.cells.iter().any(|cell| cell.name == "stale+"));
        assert!(cell_graph.module_symlinks.iter().any(|symlink| {
            symlink.entry_name == "local+"
                && symlink.source_path.as_ref() == &PathBuf::from("/tmp/slug-plan61-local-outside")
        }));

        Ok(())
    }

    #[tokio::test]
    async fn replay_input_data_rejects_wrong_workspace() -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-replay-input-workspace"),
            PathBuf::from("/tmp/slug-plan61-replay-input-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-replay-input-other"),
            PathBuf::from("/tmp/slug-plan61-replay-input-other-output"),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodCellGraphDataKey,
            Arc::new(BzlmodCellGraphDataValue::for_workspace(
                workspace_id.clone(),
                Arc::from(dice_graph::INJECTED_BZLMOD_PROJECTION_DIGEST),
                Arc::new(BzlmodCellGraphValue::empty_for_workspace(
                    workspace_id.clone(),
                )),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                other_workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        let mut dice = updater.commit().await;
        let err = dice
            .compute(&BzlmodRepoEnvKey::for_workspace_id(workspace_id.clone()))
            .await?
            .unwrap_err();
        assert!(err.to_string().contains("repo env data root"), "{err:?}");

        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                other_workspace_id,
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        let mut dice = updater.commit().await;
        let err = dice
            .compute(&BzlmodLockfileInputsKey::for_workspace_id(workspace_id))
            .await?
            .unwrap_err();
        assert!(
            err.to_string().contains("lockfile input data root"),
            "{err:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn data_only_keys_do_not_depend_on_cell_graph_workspace() -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-data-projection-workspace"),
            PathBuf::from("/tmp/slug-plan61-data-projection-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-data-projection-other"),
            PathBuf::from("/tmp/slug-plan61-data-projection-other-output"),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodCellGraphDataKey,
            Arc::new(BzlmodCellGraphDataValue::for_workspace(
                other_workspace_id.clone(),
                Arc::from(dice_graph::INJECTED_BZLMOD_PROJECTION_DIGEST),
                Arc::new(BzlmodCellGraphValue::empty_for_workspace(
                    other_workspace_id,
                )),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(BzlmodModuleVersionsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(HashMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::from([(
                    "TOKEN".to_owned(),
                    "current-workspace".to_owned(),
                )])),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoMappingsDataKey,
            Arc::new(BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(RepoMappingSnapshot::new()),
                Arc::new(RepoMappingOverrides::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRegisteredToolchainsDataKey,
            Arc::new(RegisteredToolchainsDataValue::for_workspace(
                workspace_id.clone(),
                Vec::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRegisteredExecutionPlatformsDataKey,
            Arc::new(RegisteredExecutionPlatformsDataValue::for_workspace(
                workspace_id.clone(),
                Vec::new(),
            )),
        )])?;
        let mut dice = updater.commit().await;

        let lockfile_inputs = dice
            .compute(&BzlmodLockfileInputsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        assert_eq!(lockfile_inputs.lockfile_mode, LockfileMode::Update);

        let repo_env = dice
            .compute(&BzlmodRepoEnvKey::for_workspace_id(workspace_id.clone()))
            .await??;
        assert_eq!(
            repo_env.get("TOKEN").map(String::as_str),
            Some("current-workspace")
        );

        let repo_mappings = dice
            .compute(&BzlmodRepoMappingsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        assert_eq!(repo_mappings.workspace_id, workspace_id);

        let resolution_facts = dice
            .compute(&BzlmodResolutionFactsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        assert!(resolution_facts.registry_file_hashes.is_empty());
        assert!(resolution_facts.selected_yanked_versions.is_empty());

        let registered_toolchains = dice
            .compute(&RegisteredToolchainsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        assert_eq!(registered_toolchains.workspace_id, workspace_id);

        let registered_execution_platforms = dice
            .compute(&RegisteredExecutionPlatformsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        assert_eq!(registered_execution_platforms.workspace_id, workspace_id);

        let module_versions = dice
            .compute(&ModuleVersionsKey::for_workspace_id(workspace_id))
            .await??;
        assert!(module_versions.module_versions.is_empty());
        assert_eq!(module_versions.invalidation.repo_env, *repo_env.as_ref());

        Ok(())
    }

    #[tokio::test]
    async fn cell_graph_data_rejects_wrong_workspace() -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-projection-workspace"),
            PathBuf::from("/tmp/slug-plan61-projection-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-projection-other"),
            PathBuf::from("/tmp/slug-plan61-projection-other-output"),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.set_empty_bzlmod_dice_inputs_for_workspace(workspace_id.clone())?;
        let dice = updater.commit().await;

        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(BzlmodModuleVersionsDataValue::for_workspace(
                other_workspace_id.clone(),
                Arc::new(HashMap::new()),
            )),
        )])?;
        let mut dice = updater.commit().await;
        let err = dice
            .compute(&ModuleVersionsKey::for_workspace_id(workspace_id.clone()))
            .await?
            .unwrap_err();
        assert!(
            err.to_string().contains("module versions data root"),
            "{err:?}"
        );

        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(BzlmodModuleVersionsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(HashMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                other_workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        let mut dice = updater.commit().await;
        let err = dice
            .compute(&ModuleVersionsKey::for_workspace_id(workspace_id.clone()))
            .await?
            .unwrap_err();
        assert!(
            err.to_string().contains("resolution facts data root"),
            "{err:?}"
        );

        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRegisteredToolchainsDataKey,
            Arc::new(RegisteredToolchainsDataValue::for_workspace(
                other_workspace_id.clone(),
                Vec::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRegisteredExecutionPlatformsDataKey,
            Arc::new(RegisteredExecutionPlatformsDataValue::for_workspace(
                other_workspace_id,
                Vec::new(),
            )),
        )])?;
        let mut dice = updater.commit().await;
        let err = dice
            .compute(&RegisteredToolchainsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await?
            .unwrap_err();
        assert!(
            err.to_string().contains("registered toolchain data root"),
            "{err:?}"
        );
        let err = dice
            .compute(&RegisteredExecutionPlatformsKey::for_workspace_id(
                workspace_id,
            ))
            .await?
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("registered execution platform data root"),
            "{err:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn current_workspace_helpers_use_cell_graph_workspace_id() -> slug_error::Result<()> {
        let project_root = PathBuf::from("/tmp/slug-plan61-current-workspace-helper");
        let workspace_id = WorkspaceId::new(
            project_root.clone(),
            PathBuf::from("/tmp/slug-plan61-current-workspace-output-base"),
        );
        let data = BzlmodCellGraphValue::empty_for_workspace(workspace_id.clone());

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.set_empty_bzlmod_dice_inputs_for_workspace(data.workspace_id)?;
        let mut dice = updater.commit().await;

        let module_versions = module_versions_for_current_workspace(&mut dice).await?;
        let registered_toolchains = registered_toolchains_for_current_workspace(&mut dice).await?;
        let registered_execution_platforms =
            registered_execution_platforms_for_current_workspace(&mut dice).await?;

        assert_eq!(module_versions.workspace_id, workspace_id);
        assert_eq!(registered_toolchains.workspace_id, workspace_id);
        assert_eq!(registered_execution_platforms.workspace_id, workspace_id);
        assert_ne!(
            module_versions.workspace_id,
            WorkspaceId::for_project_root(project_root)
        );

        Ok(())
    }

    #[tokio::test]
    async fn data_only_current_workspace_helpers_do_not_require_cell_graph()
    -> slug_error::Result<()> {
        let project_root = PathBuf::from("/tmp/slug-plan61-data-current-workspace-helper");
        let workspace_id = WorkspaceId::new(
            project_root.clone(),
            PathBuf::from("/tmp/slug-plan61-data-current-workspace-output-base"),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.changed_to(vec![(
            BzlmodModuleVersionsDataKey,
            Arc::new(
                BzlmodModuleVersionsDataValue::for_workspace_with_root_module_name(
                    workspace_id.clone(),
                    "root".to_owned(),
                    Arc::new(HashMap::new()),
                ),
            ),
        )])?;
        updater.changed_to(vec![(
            BzlmodLockfileInputsDataKey,
            Arc::new(BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoEnvDataKey,
            Arc::new(BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRepoMappingsDataKey,
            Arc::new(BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(RepoMappingSnapshot::new()),
                Arc::new(RepoMappingOverrides::new()),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodResolutionFactsDataKey,
            Arc::new(BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRegisteredToolchainsDataKey,
            Arc::new(RegisteredToolchainsDataValue::for_workspace(
                workspace_id.clone(),
                Vec::new(),
            )),
        )])?;
        updater.changed_to(vec![(
            BzlmodRegisteredExecutionPlatformsDataKey,
            Arc::new(RegisteredExecutionPlatformsDataValue::for_workspace(
                workspace_id.clone(),
                Vec::new(),
            )),
        )])?;
        let mut dice = updater.commit().await;

        let module_versions = module_versions_for_current_workspace(&mut dice).await?;
        let registered_toolchains = registered_toolchains_for_current_workspace(&mut dice).await?;
        let registered_execution_platforms =
            registered_execution_platforms_for_current_workspace(&mut dice).await?;
        let current_workspace = bzlmod_workspace_id_for_current_workspace(&mut dice).await?;

        assert_eq!(module_versions.workspace_id, workspace_id);
        assert_eq!(registered_toolchains.workspace_id, workspace_id);
        assert_eq!(registered_execution_platforms.workspace_id, workspace_id);
        assert_eq!(current_workspace, workspace_id);
        assert_ne!(
            module_versions.workspace_id,
            WorkspaceId::for_project_root(project_root)
        );

        Ok(())
    }
}
