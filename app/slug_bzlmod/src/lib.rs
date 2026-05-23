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
pub mod extension_execution_dice;
pub mod extensions;
pub mod fetch;
pub mod globals;
pub mod integrity;
pub mod lockfile;
pub mod module_extension_executor;
pub mod parser;
pub mod pending_repo_cells;
pub mod registry;
pub mod repo_mapping;
pub mod repo_spec;
pub mod repository_execution;
pub mod repository_executor;
pub mod repository_invocations;
pub mod resolution;
pub mod spoke_materialization;
pub mod starlark_repo_rule_executor;
pub mod types;
pub mod version;

// ============================================================================
// Module version registry
// ============================================================================
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
pub use cache::ModuleCache;
pub use dice_graph::BzlmodCellGraphKey;
pub use dice_graph::BzlmodCommandPolicyKey;
pub use dice_graph::BzlmodCommandPolicyValue;
pub use dice_graph::BzlmodEventCounters;
pub use dice_graph::BzlmodEventKind;
pub use dice_graph::BzlmodResolutionKey;
pub use dice_graph::BzlmodWorkspaceKey;
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
pub use dice_graph::RegisteredExecutionPlatformsKey;
pub use dice_graph::RegisteredExecutionPlatformsValue;
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
use dupe::Dupe;
pub use extension_execution_dice::ModuleExtensionError;
pub use extension_execution_dice::ModuleExtensionExecutionKey;
pub use extension_execution_dice::ModuleExtensionResult;
pub use extension_execution_dice::build_canonical_names;
pub use extension_execution_dice::compute_bzl_transitive_digest;
pub use extension_execution_dice::compute_bzl_transitive_digest_for_project;
pub use extension_execution_dice::compute_bzl_transitive_digest_for_project_with_repo_mappings;
pub use extension_execution_dice::create_extension_execution_key;
pub use extension_execution_dice::extension_spokes_key_for_canonical_repo;
pub use extension_execution_dice::extension_spokes_key_for_extension_id;
pub use extension_execution_dice::extract_extension_name;
pub use extension_execution_dice::extract_owning_module;
pub use extensions::AggregatedExtension;
pub use extensions::aggregate_extensions;
pub use extensions::aggregate_extensions_with_policy;
pub use extensions::aggregate_extensions_with_root;
pub use extensions::canonical_extension_id;
pub use extensions::compute_extension_input_hash;
pub use fetch::SourceFetcher;
pub use integrity::verify_integrity;
pub use lockfile::Lockfile;
pub use lockfile::LockfileMode;
pub use lockfile::lockfile_path;
pub use lockfile::read_hidden_lockfile_path;
pub use lockfile::read_lockfile_path_with_mode;
pub use lockfile::read_lockfile_with_mode;
pub use module_extension_executor::ExtensionExecutionOutput;
pub use module_extension_executor::MODULE_EXTENSION_EXECUTOR_IMPL;
pub use module_extension_executor::ModuleExtensionExecutorImpl;
pub use module_extension_executor::ModuleExtensionMetadata;
pub use parser::ModuleFileParseSession;
pub use parser::ParsedModuleFileWithInputs;
pub use parser::include_label_to_path;
pub use parser::parse_module_bazel;
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
pub use repository_execution::RepositoryRegistry;
pub use repository_execution::RepositoryRuleExecutionKey;
pub use repository_execution::RepositoryRuleResult;
pub use repository_execution::repo_spec_to_invocation;
pub use repository_execution::repository_recorded_inputs_current;
pub use repository_executor::execute_repository_rule;
pub use repository_executor::repo_layout_is_valid_for_invocation;
pub use repository_executor::repository_output_digest;
pub use repository_invocations::AttrValue as RepoAttrValue;
pub use repository_invocations::RegistryGuard;
pub use repository_invocations::RepositoryInvocation;
pub use repository_invocations::RepositoryInvocationRegistry;
pub use repository_invocations::record_invocation;
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
pub use types::Module;
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

/// Bzlmod facts produced by startup resolution and injected into DICE for the
/// current command. This is the transitional Plan 61 boundary between legacy
/// cell parsing and DICE-owned bzlmod values.
#[derive(Debug, Clone, Default, PartialEq, Eq, Allocative)]
pub struct BzlmodSessionData {
    pub module_versions: HashMap<String, String>,
    pub registered_toolchains: Vec<RegisteredToolchain>,
    pub registered_execution_platforms: Vec<String>,
    pub extension_aggregations: HashMap<String, AggregatedExtension>,
    pub root_module_name: String,
    pub project_root: PathBuf,
    pub hidden_lockfile_path: Option<PathBuf>,
    pub visible_lockfile_digest: Option<String>,
    pub hidden_lockfile_digest: Option<String>,
    pub lockfile_mode: LockfileMode,
    pub repo_env: BTreeMap<String, String>,
    pub registry_file_hashes: indexmap::IndexMap<String, String>,
    pub selected_yanked_versions: indexmap::IndexMap<String, String>,
    pub repo_mappings: RepoMappingSnapshot,
    pub repo_mapping_overrides: RepoMappingOverrides,
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
#[display("BzlmodSessionDataKey")]
pub struct BzlmodSessionDataKey;

impl dice::InjectedKey for BzlmodSessionDataKey {
    type Value = Arc<BzlmodSessionData>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

pub trait SetBzlmodSessionData {
    fn set_bzlmod_session_data(&mut self, data: BzlmodSessionData) -> slug_error::Result<()>;
}

impl SetBzlmodSessionData for dice::DiceTransactionUpdater {
    fn set_bzlmod_session_data(&mut self, data: BzlmodSessionData) -> slug_error::Result<()> {
        Ok(self.changed_to(vec![(BzlmodSessionDataKey, Arc::new(data))])?)
    }
}
