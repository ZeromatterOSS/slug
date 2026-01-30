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
pub mod extensions;
pub mod fetch;
pub mod globals;
pub mod integrity;
pub mod lockfile;
pub mod parser;
pub mod registry;
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
pub use repository_execution::RepositoryRegistry;
pub use repository_execution::RepositoryRuleExecutionKey;
pub use repository_execution::RepositoryRuleResult;
pub use repository_executor::execute_repository_rule;
pub use repository_invocations::AttrValue as RepoAttrValue;
pub use repository_invocations::RegistryGuard;
pub use repository_invocations::RepositoryInvocation;
pub use repository_invocations::RepositoryInvocationRegistry;
pub use repository_invocations::record_invocation;
pub use types::BazelDep;
pub use types::Module;
pub use version::Version;
