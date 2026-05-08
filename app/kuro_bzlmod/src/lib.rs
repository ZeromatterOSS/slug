/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bzlmod (Bazel module) implementation for Kuro.
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
use std::collections::HashMap;
use std::sync::RwLock;

pub use cache::ModuleCache;
pub use extension_execution_dice::ModuleExtensionError;
pub use extension_execution_dice::ModuleExtensionExecutionKey;
pub use extension_execution_dice::ModuleExtensionResult;
pub use extension_execution_dice::build_canonical_names;
pub use extension_execution_dice::compute_bzl_transitive_digest;
pub use extension_execution_dice::create_extension_execution_key;
pub use extension_execution_dice::extract_extension_name;
pub use extension_execution_dice::extract_owning_module;
pub use extension_execution_dice::set_extension_aggregations;
pub use extensions::AggregatedExtension;
pub use extensions::aggregate_extensions;
pub use extensions::aggregate_extensions_with_root;
pub use extensions::compute_extension_input_hash;
pub use fetch::SourceFetcher;
pub use integrity::verify_integrity;
pub use lockfile::Lockfile;
pub use lockfile::LockfileMode;
pub use lockfile::cached_lockfile;
pub use lockfile::invalidate_cached_lockfile;
pub use lockfile::lockfile_path;
pub use module_extension_executor::ExtensionExecutionOutput;
pub use module_extension_executor::MODULE_EXTENSION_EXECUTOR_IMPL;
pub use module_extension_executor::ModuleExtensionExecutorImpl;
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
// consumers that already do `use kuro_bzlmod::RegisteredToolchain`.
pub use registry::RegistryClient;
pub use repo_mapping::BzlmodRepoMapping;
pub use repo_mapping::CanonicalLabel;
pub use repo_mapping::CanonicalRepoName;
pub use repo_mapping::ExtensionImportCanonicalization;
pub use repo_mapping::canonical_repo_for_extension_import;
pub use repo_mapping::canonicalize_label_with_package_context;
pub use repo_spec::RepoSpec;
pub use repo_spec::in_extension_context;
pub use repo_spec::record_repo_spec;
pub use repo_spec::with_repo_spec_registry;
pub use repository_execution::ExtensionRepoExecutionKey;
pub use repository_execution::RepositoryRegistry;
pub use repository_execution::RepositoryRuleExecutionKey;
pub use repository_execution::RepositoryRuleResult;
pub use repository_execution::repo_spec_to_invocation;
pub use repository_executor::execute_repository_rule;
pub use repository_invocations::AttrValue as RepoAttrValue;
pub use repository_invocations::RegistryGuard;
pub use repository_invocations::RepositoryInvocation;
pub use repository_invocations::RepositoryInvocationRegistry;
pub use repository_invocations::record_invocation;
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
pub use resolution::resolve_all_dependencies;
pub use resolution::resolve_local_modules;
pub use resolution::resolve_local_override;
pub use resolution::resolve_with_lockfile;
pub use spoke_materialization::SpokeRegistration;
pub use spoke_materialization::extension_spokes_seeded;
pub use spoke_materialization::lookup_spoke;
pub use spoke_materialization::mark_extension_spokes_seeded;
pub use spoke_materialization::materialize_spoke_sync;
pub use spoke_materialization::register_spoke;
pub use spoke_materialization::with_extension_dice;
pub use starlark_repo_rule_executor::STARLARK_REPO_RULE_EXECUTOR_IMPL;
pub use starlark_repo_rule_executor::StarlarkRepoRuleExecutorImpl;
pub use starlark_repo_rule_executor::is_builtin_repo_rule;
pub use types::BazelDep;
pub use types::Module;
pub use types::UseRepo;
pub use version::Version;

/// Global registry mapping cell/module names to their resolved versions.
/// Populated during bzlmod resolution, read by module_version() builtin.
static MODULE_VERSIONS: RwLock<Option<HashMap<String, String>>> = RwLock::new(None);

/// Set the module version map (module_name -> version string).
/// Called after bzlmod resolution completes.
pub fn set_module_versions(versions: HashMap<String, String>) {
    if let Ok(mut map) = MODULE_VERSIONS.write() {
        *map = if versions.is_empty() {
            None
        } else {
            Some(versions)
        };
    }
}

/// Get the version of a module by its cell/module name.
/// Returns None if no version is known for this cell.
pub fn get_module_version(cell_name: &str) -> Option<String> {
    MODULE_VERSIONS
        .read()
        .ok()
        .and_then(|map| map.as_ref().and_then(|m| m.get(cell_name).cloned()))
}

// ============================================================================
// Toolchain and Execution Platform Registrations
// ============================================================================

/// A registered toolchain entry, tracking its origin module so Plan 13
/// Phase 3's lazy fallback can filter the deferred pool by relevance.
#[derive(Debug, Clone)]
pub struct RegisteredToolchain {
    /// Origin module name (root module is marked `is_root = true`).
    pub module: String,
    /// The label string passed to `register_toolchains()`.
    pub label: String,
    /// True iff this registration came from the root module.
    pub is_root: bool,
}

/// Global priority-ordered list of registered toolchains.
/// Populated during cell resolution from register_toolchains() calls in MODULE.bazel.
/// Order: root module first, then BFS order of dep graph (Bazel priority).
static REGISTERED_TOOLCHAINS: RwLock<Vec<RegisteredToolchain>> = RwLock::new(Vec::new());

/// Global priority-ordered list of registered execution platforms.
static REGISTERED_EXECUTION_PLATFORMS: RwLock<Vec<String>> = RwLock::new(Vec::new());

/// Set the global ordered list of registered toolchains.
/// Called after bzlmod resolution collects registrations from all modules.
pub fn set_registered_toolchains(toolchains: Vec<RegisteredToolchain>) {
    if let Ok(mut guard) = REGISTERED_TOOLCHAINS.write() {
        *guard = toolchains;
    }
}

/// Set the global ordered list of registered execution platforms.
pub fn set_registered_execution_platforms(platforms: Vec<String>) {
    if let Ok(mut guard) = REGISTERED_EXECUTION_PLATFORMS.write() {
        *guard = platforms;
    }
}

/// Get the priority-ordered list of registered toolchains.
pub fn get_registered_toolchains() -> Vec<RegisteredToolchain> {
    REGISTERED_TOOLCHAINS
        .read()
        .ok()
        .map(|v| v.clone())
        .unwrap_or_default()
}

/// Get the priority-ordered list of registered execution platform labels.
pub fn get_registered_execution_platforms() -> Vec<String> {
    REGISTERED_EXECUTION_PLATFORMS
        .read()
        .ok()
        .map(|v| v.clone())
        .unwrap_or_default()
}
