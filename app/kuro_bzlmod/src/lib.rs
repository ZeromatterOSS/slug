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
pub mod repo_spec;
pub mod repository_execution;
pub mod repository_executor;
pub mod repository_invocations;
pub mod resolution;
pub mod synthetic_repos;
pub mod types;
pub mod version;

pub use cache::ModuleCache;
pub use fetch::SourceFetcher;
pub use integrity::verify_integrity;
pub use lockfile::DownloadedFileLockEntry;
pub use lockfile::Lockfile;
pub use lockfile::LockfileMode;
pub use lockfile::RepositoryRuleLockEntry;
pub use lockfile::lockfile_path;
pub use parser::parse_module_bazel;
pub use registry::RegistryClient;
pub use registry::DEFAULT_REGISTRY_URL;
pub use resolution::resolve_all_dependencies;
pub use resolution::resolve_local_modules;
pub use resolution::resolve_local_override;
pub use resolution::resolve_with_lockfile;
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
pub use extension_execution_dice::ModuleExtensionError;
pub use extension_execution_dice::ModuleExtensionExecutionKey;
pub use extension_execution_dice::ModuleExtensionResult;
pub use extension_execution_dice::build_canonical_names;
pub use extension_execution_dice::compute_bzl_transitive_digest;
pub use extension_execution_dice::extract_extension_name;
pub use extensions::AggregatedExtension;
pub use extensions::compute_extension_input_hash;
pub use extensions::aggregate_extensions;
pub use repository_execution::ExtensionRepoExecutionKey;
pub use repository_execution::RepositoryRegistry;
pub use repository_execution::RepositoryRuleExecutionKey;
pub use repository_execution::RepositoryRuleResult;
pub use repository_execution::repo_spec_to_invocation;
pub use repository_executor::execute_repository_rule;
pub use repo_spec::RepoSpec;
pub use repo_spec::in_extension_context;
pub use repo_spec::record_repo_spec;
pub use repo_spec::with_repo_spec_registry;
pub use repository_invocations::AttrValue as RepoAttrValue;
pub use repository_invocations::RegistryGuard;
pub use repository_invocations::RepositoryInvocation;
pub use repository_invocations::RepositoryInvocationRegistry;
pub use repository_invocations::record_invocation;
pub use pending_repo_cells::ExtensionCellDefinitions;
pub use pending_repo_cells::PendingRepoCell;
pub use pending_repo_cells::RepoAlias;
pub use pending_repo_cells::build_extension_cells;
pub use pending_repo_cells::build_extension_cell_definitions;
pub use pending_repo_cells::build_all_extension_cells;
pub use pending_repo_cells::build_use_repo_aliases;
pub use pending_repo_cells::extract_use_repos_for_extension;
pub use pending_repo_cells::is_extension_repo_canonical_name;
pub use pending_repo_cells::parse_canonical_name;
pub use types::BazelDep;
pub use types::Module;
pub use types::UseRepo;
pub use version::Version;
pub use module_extension_executor::ExtensionExecutionOutput;
pub use module_extension_executor::ModuleExtensionExecutorImpl;
pub use module_extension_executor::MODULE_EXTENSION_EXECUTOR_IMPL;
