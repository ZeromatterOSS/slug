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
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
pub use cache::ModuleCache;
pub use dice_graph::BzlmodCellGraphAlias;
pub use dice_graph::BzlmodCellGraphCell;
pub use dice_graph::BzlmodCellGraphDataKey;
pub use dice_graph::BzlmodCellGraphDynamicAlias;
pub use dice_graph::BzlmodCellGraphExtensionCell;
pub use dice_graph::BzlmodCellGraphKey;
pub use dice_graph::BzlmodCellGraphModuleSetup;
pub use dice_graph::BzlmodCellGraphModuleSymlink;
pub use dice_graph::BzlmodCellGraphScopedAlias;
pub use dice_graph::BzlmodCellGraphValue;
pub use dice_graph::BzlmodCommandPolicyKey;
pub use dice_graph::BzlmodCommandPolicyValue;
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
pub use dice_graph::RegisteredExecutionPlatformsDataValue;
pub use dice_graph::RegisteredExecutionPlatformsKey;
pub use dice_graph::RegisteredExecutionPlatformsValue;
pub use dice_graph::RegisteredToolchainsDataValue;
pub use dice_graph::RegisteredToolchainsKey;
pub use dice_graph::RegisteredToolchainsValue;
pub use dice_graph::RepoMappingKey;
pub use dice_graph::RepoMappingScope;
pub use dice_graph::RepoMaterializationManifestKey;
pub use dice_graph::RepoMaterializationManifestValue;
pub use dice_graph::RepoOriginKind;
pub use dice_graph::ResolvedModuleIdentity;
pub use dice_graph::RootModuleFileValue;
pub use dice_graph::WorkspaceId;
pub use dice_graph::bzlmod_event_counters;
pub use dice_graph::module_file_inputs_digest;
pub use dice_graph::record_bzlmod_event;
pub use dice_graph::repo_env_policy_digest;
pub use extension_execution_dice::BzlLoadLocation;
pub use extension_execution_dice::ModuleExtensionError;
pub use extension_execution_dice::ModuleExtensionExecutionKey;
pub use extension_execution_dice::ModuleExtensionResult;
pub use extension_execution_dice::compute_bzl_transitive_digest;
pub use extension_execution_dice::compute_bzl_transitive_digest_for_project_with_repo_mappings;
pub use extension_execution_dice::compute_bzl_transitive_digest_from_file_contents;
pub use extension_execution_dice::extension_bzl_location_under_project;
pub use extension_execution_dice::extract_extension_name;
pub use extension_execution_dice::extract_owning_module;
pub use extension_execution_dice::label_bzl_location_under_project;
pub use extension_execution_dice::literal_loads;
pub use extensions::AggregatedExtension;
pub use extensions::ExtensionResult;
pub use extensions::GeneratedRepo;
pub use extensions::ModuleInfo;
pub use extensions::aggregate_extensions;
pub use extensions::aggregate_extensions_with_policy;
pub use extensions::canonical_extension_id;
pub use extensions::compute_extension_input_hash;
pub use fetch::SourceFetcher;
pub use integrity::verify_integrity;
pub use lockfile::Lockfile;
pub use lockfile::LockfileMode;
pub use lockfile::compute_sha256_hex;
pub use lockfile::compute_sri_hash;
pub use lockfile::lockfile_path;
pub use lockfile::parse_lockfile_content;
pub use lockfile::read_hidden_lockfile_path;
pub use lockfile::read_lockfile_with_mode;
pub use lockfile::recorded_dirents_input;
pub use lockfile::recorded_dirtree_input;
pub use lockfile::recorded_env_input;
pub use lockfile::recorded_file_input;
pub use lockfile::recorded_file_input_with_recorded_path;
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
pub use pending_repo_cells::pre_compute_extension_repo_cells_from_lockfile;
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
pub use resolution::ResolvedLocalModules;
pub use resolution::ResolvedModuleInfo;
pub use resolution::ResolvedRemoteModule;
pub use resolution::ResolvedRemoteModules;
pub use resolution::parse_allowed_yanked_versions;
pub use resolution::resolve_all_dependencies;
pub use resolution::resolve_local_modules;
pub use resolution::resolve_local_override;
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

/// Transitional bzlmod payload used to update the injected DICE projections for
/// the current command while the legacy resolver is being decomposed into
/// DICE-owned graph keys.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct BzlmodProjectionData {
    pub module_versions: BzlmodModuleVersionsDataValue,
    pub registered_toolchains: RegisteredToolchainsDataValue,
    pub registered_execution_platforms: RegisteredExecutionPlatformsDataValue,
    pub extension_aggregations: BzlmodExtensionAggregationsDataValue,
    pub lockfile_inputs: BzlmodLockfileInputsDataValue,
    pub repo_env: BzlmodRepoEnvDataValue,
    pub resolution_facts: BzlmodResolutionFactsValue,
    pub repo_mappings: BzlmodRepoMappingsDataValue,
    pub cell_graph: BzlmodCellGraphValue,
}

impl BzlmodProjectionData {
    pub fn for_workspace(workspace_id: WorkspaceId) -> Self {
        Self {
            module_versions: BzlmodModuleVersionsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(HashMap::new()),
            ),
            registered_toolchains: RegisteredToolchainsDataValue::for_workspace(
                workspace_id.clone(),
                Vec::new(),
            ),
            registered_execution_platforms: RegisteredExecutionPlatformsDataValue::for_workspace(
                workspace_id.clone(),
                Vec::new(),
            ),
            extension_aggregations: BzlmodExtensionAggregationsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(HashMap::new()),
            ),
            lockfile_inputs: BzlmodLockfileInputsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BzlmodLockfileInputsValue::default()),
            ),
            repo_env: BzlmodRepoEnvDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(BTreeMap::new()),
            ),
            resolution_facts: BzlmodResolutionFactsValue::for_workspace(
                workspace_id.clone(),
                indexmap::IndexMap::new(),
                indexmap::IndexMap::new(),
            ),
            repo_mappings: BzlmodRepoMappingsDataValue::for_workspace(
                workspace_id.clone(),
                Arc::new(RepoMappingSnapshot::new()),
                Arc::new(RepoMappingOverrides::new()),
            ),
            cell_graph: BzlmodCellGraphValue::empty_for_workspace(workspace_id),
        }
    }

    pub fn empty_no_project_sentinel() -> Self {
        Self::for_workspace(WorkspaceId::new(
            PathBuf::new(),
            PathBuf::from("buck-out/v2"),
        ))
    }

    #[cfg(test)]
    pub fn empty_for_project_root(project_root: PathBuf) -> Self {
        Self::for_workspace(WorkspaceId::for_project_root(project_root))
    }
}

pub trait SetBzlmodProjectionData {
    fn set_bzlmod_projection_data(&mut self, data: BzlmodProjectionData) -> slug_error::Result<()>;
}

fn validate_projection_workspace(
    field: &str,
    cell_graph_workspace_id: &WorkspaceId,
    data_workspace_id: &WorkspaceId,
) -> slug_error::Result<()> {
    if data_workspace_id != cell_graph_workspace_id {
        return Err(slug_error::slug_error!(
            slug_error::ErrorTag::Tier0,
            "BzlmodProjectionData carries {} for project root '{}', \
             but its cell graph root is '{}'",
            field,
            data_workspace_id.canonical_project_root.display(),
            cell_graph_workspace_id.canonical_project_root.display()
        ));
    }
    Ok(())
}

impl SetBzlmodProjectionData for dice::DiceTransactionUpdater {
    fn set_bzlmod_projection_data(&mut self, data: BzlmodProjectionData) -> slug_error::Result<()> {
        let cell_graph_workspace_id = &data.cell_graph.workspace_id;
        validate_projection_workspace(
            "module-version data",
            cell_graph_workspace_id,
            &data.module_versions.workspace_id,
        )?;
        validate_projection_workspace(
            "registered-toolchain data",
            cell_graph_workspace_id,
            &data.registered_toolchains.workspace_id,
        )?;
        validate_projection_workspace(
            "registered-execution-platform data",
            cell_graph_workspace_id,
            &data.registered_execution_platforms.workspace_id,
        )?;
        validate_projection_workspace(
            "extension-aggregation data",
            cell_graph_workspace_id,
            &data.extension_aggregations.workspace_id,
        )?;
        validate_projection_workspace(
            "lockfile-input data",
            cell_graph_workspace_id,
            &data.lockfile_inputs.workspace_id,
        )?;
        validate_projection_workspace(
            "repo-env data",
            cell_graph_workspace_id,
            &data.repo_env.workspace_id,
        )?;
        validate_projection_workspace(
            "resolution-facts data",
            cell_graph_workspace_id,
            &data.resolution_facts.workspace_id,
        )?;
        validate_projection_workspace(
            "repo-mapping data",
            cell_graph_workspace_id,
            &data.repo_mappings.workspace_id,
        )?;

        let lockfile_inputs_data = Arc::new(data.lockfile_inputs.clone());
        let repo_env_data = Arc::new(data.repo_env.clone());
        let module_versions = Arc::new(data.module_versions.clone());
        let resolution_facts = Arc::new(data.resolution_facts.clone());
        let repo_mappings = Arc::new(data.repo_mappings.clone());
        let extension_aggregations = Arc::new(data.extension_aggregations.clone());
        let cell_graph = Arc::new(data.cell_graph.clone());
        let registered_toolchains = Arc::new(data.registered_toolchains.clone());
        let registered_execution_platforms = Arc::new(data.registered_execution_platforms.clone());
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
        self.changed_to(vec![(BzlmodCellGraphDataKey, cell_graph)])?;
        Ok(())
    }
}

pub async fn module_versions_for_current_workspace(
    ctx: &mut dice::DiceComputations<'_>,
) -> slug_error::Result<Arc<ModuleVersionsValue>> {
    let cell_graph = ctx.compute(&BzlmodCellGraphDataKey).await?;
    let key = ModuleVersionsKey::for_workspace_id(cell_graph.workspace_id.clone());
    ctx.compute(&key).await?
}

pub async fn registered_toolchains_for_current_workspace(
    ctx: &mut dice::DiceComputations<'_>,
) -> slug_error::Result<Arc<RegisteredToolchainsValue>> {
    let cell_graph = ctx.compute(&BzlmodCellGraphDataKey).await?;
    let key = RegisteredToolchainsKey::for_workspace_id(cell_graph.workspace_id.clone());
    ctx.compute(&key).await?
}

pub async fn registered_execution_platforms_for_current_workspace(
    ctx: &mut dice::DiceComputations<'_>,
) -> slug_error::Result<Arc<RegisteredExecutionPlatformsValue>> {
    let cell_graph = ctx.compute(&BzlmodCellGraphDataKey).await?;
    let key = RegisteredExecutionPlatformsKey::for_workspace_id(cell_graph.workspace_id.clone());
    ctx.compute(&key).await?
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[tokio::test]
    async fn set_bzlmod_projection_data_uses_projection_workspace_id() -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-projection-workspace"),
            PathBuf::from("/tmp/slug-plan61-custom-output-base"),
        );
        let mut data = BzlmodProjectionData::for_workspace(workspace_id.clone());
        data.cell_graph = BzlmodCellGraphValue {
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
        updater.set_bzlmod_projection_data(data)?;
        let mut dice = updater.commit().await;

        let repo_mappings = dice
            .compute(&BzlmodRepoMappingsKey::for_workspace_id(
                workspace_id.clone(),
            ))
            .await??;
        assert!(repo_mappings.repo_mappings.is_empty());
        let cell_graph = dice
            .compute(&BzlmodCellGraphKey::for_workspace_id(workspace_id.clone()))
            .await??;
        assert_eq!(cell_graph.workspace_id, workspace_id);
        assert_eq!(cell_graph.root_module_name, "root_mod");
        assert_eq!(cell_graph.cells[0].name, "root_mod");
        assert_eq!(cell_graph.root_aliases[0].apparent_name, "dep");
        let module_versions = dice
            .compute(&ModuleVersionsKey::for_workspace_id(workspace_id))
            .await??;
        assert_eq!(module_versions.invalidation.root_module_name, "root_mod");

        Ok(())
    }

    #[tokio::test]
    async fn set_bzlmod_projection_data_rejects_mismatched_workspace_provenance()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-projection-consistency-workspace"),
            PathBuf::from("/tmp/slug-plan61-projection-consistency-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-projection-consistency-other"),
            PathBuf::from("/tmp/slug-plan61-projection-consistency-other-output"),
        );
        let mut data = BzlmodProjectionData::for_workspace(workspace_id);
        data.repo_env =
            BzlmodRepoEnvDataValue::for_workspace(other_workspace_id, Arc::new(BTreeMap::new()));

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        let err = updater.set_bzlmod_projection_data(data).unwrap_err();
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
    async fn set_bzlmod_projection_data_rejects_mismatched_lockfile_input_provenance()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-lockfile-provenance-workspace"),
            PathBuf::from("/tmp/slug-plan61-lockfile-provenance-output-base"),
        );
        let other_workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-lockfile-provenance-other"),
            PathBuf::from("/tmp/slug-plan61-lockfile-provenance-other-output"),
        );
        let mut data = BzlmodProjectionData::for_workspace(workspace_id);
        data.lockfile_inputs = BzlmodLockfileInputsDataValue::for_workspace(
            other_workspace_id,
            Arc::new(BzlmodLockfileInputsValue::default()),
        );

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        let err = updater.set_bzlmod_projection_data(data).unwrap_err();
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
    async fn set_bzlmod_projection_data_derives_lockfile_digests_from_values()
    -> slug_error::Result<()> {
        let workspace_id = WorkspaceId::new(
            PathBuf::from("/tmp/slug-plan61-projection-lockfile-digest"),
            PathBuf::from("/tmp/slug-plan61-projection-lockfile-output-base"),
        );
        let mut data = BzlmodProjectionData::for_workspace(workspace_id.clone());
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
        data.lockfile_inputs = BzlmodLockfileInputsDataValue::for_workspace(
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
        data.repo_env = BzlmodRepoEnvDataValue::for_workspace(
            workspace_id.clone(),
            Arc::new(BTreeMap::from([(
                "TOKEN".to_owned(),
                "from-projection".to_owned(),
            )])),
        );
        data.resolution_facts.registry_file_hashes.insert(
            "registry/modules/dep/1.0/MODULE.bazel".to_owned(),
            "sha256-registry".to_owned(),
        );
        data.resolution_facts
            .selected_yanked_versions
            .insert("dep@1.0".to_owned(), "allowed by flag".to_owned());

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.set_bzlmod_projection_data(data)?;
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
    async fn semantic_projection_keys_use_cell_graph_workspace() -> slug_error::Result<()> {
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
            Arc::new(BzlmodCellGraphValue::empty_for_workspace(
                workspace_id.clone(),
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
            Arc::new(BzlmodCellGraphValue::empty_for_workspace(
                workspace_id.clone(),
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
    async fn data_only_projection_keys_do_not_depend_on_cell_graph_workspace()
    -> slug_error::Result<()> {
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
            Arc::new(BzlmodCellGraphValue::empty_for_workspace(
                other_workspace_id,
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

        let err = dice
            .compute(&ModuleVersionsKey::for_workspace_id(workspace_id))
            .await?
            .unwrap_err();
        assert!(
            err.to_string().contains("bzlmod cell graph root"),
            "{err:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn projection_data_rejects_wrong_workspace() -> slug_error::Result<()> {
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
        updater.set_bzlmod_projection_data(BzlmodProjectionData::for_workspace(
            workspace_id.clone(),
        ))?;
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
    async fn current_workspace_helpers_use_projection_workspace_id() -> slug_error::Result<()> {
        let project_root = PathBuf::from("/tmp/slug-plan61-current-workspace-helper");
        let workspace_id = WorkspaceId::new(
            project_root.clone(),
            PathBuf::from("/tmp/slug-plan61-current-workspace-output-base"),
        );
        let data = BzlmodProjectionData::for_workspace(workspace_id.clone());

        let dice = dice::testing::DiceBuilder::new()
            .build(dice::UserComputationData::new())
            .unwrap()
            .commit()
            .await;
        let mut updater = dice.into_updater();
        updater.set_bzlmod_projection_data(data)?;
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
}
