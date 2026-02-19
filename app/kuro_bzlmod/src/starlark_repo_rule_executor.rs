/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Late binding trait for Starlark repository rule execution.
//!
//! This module provides the interface for executing custom Starlark `repository_rule`
//! implementations. The actual implementation lives in `kuro_interpreter_for_build`
//! which has access to the Starlark interpreter.
//!
//! ## Why Late Binding?
//!
//! The crate dependency direction is:
//!   `kuro_interpreter_for_build` depends on `kuro_bzlmod`
//!
//! But custom repository rule execution needs to:
//!   1. Load the rule's `.bzl` file via DICE
//!   2. Get the `FrozenStarlarkRepositoryRule` from the loaded module
//!   3. Create a `RepositoryContext` with the working directory and attrs
//!   4. Execute `rule.implementation(ctx)` in Starlark
//!
//! The late binding pattern allows `ExtensionRepoExecutionKey::compute()` in
//! `kuro_bzlmod` to call into `kuro_interpreter_for_build` without a direct dependency.

use std::path::Path;

use async_trait::async_trait;
use dice::DiceComputations;
use kuro_util::late_binding::LateBinding;

use crate::repository_invocations::RepositoryInvocation;

/// Names of built-in repository rules implemented natively (not via Starlark execution).
///
/// These rules are handled by `execute_repository_rule()` in `repository_executor.rs`
/// and do NOT need Starlark implementation lookup.
pub const BUILTIN_REPO_RULES: &[&str] = &[
    "http_archive",
    "http_file",
    "git_repository",
    "new_git_repository",
    "local_repository",
    "new_local_repository",
];

/// Check if a rule name is a built-in repo rule that should be executed natively.
pub fn is_builtin_repo_rule(rule_name: &str) -> bool {
    BUILTIN_REPO_RULES.contains(&rule_name)
}

/// Trait for Starlark repository rule execution.
///
/// Implementations provide the actual Starlark execution logic. The implementation
/// lives in `kuro_interpreter_for_build` where it can access:
/// - The Starlark interpreter
/// - DICE for loading `.bzl` files
/// - `RepositoryContext` for passing to the implementation function
#[async_trait]
pub trait StarlarkRepoRuleExecutorImpl: Send + Sync + 'static {
    /// Execute a Starlark repository rule implementation.
    ///
    /// This method:
    /// 1. Parses the `rule_bzl_path` and loads the module via DICE
    /// 2. Gets the `FrozenStarlarkRepositoryRule` by name
    /// 3. Creates a `RepositoryContext` with the attrs and working directory
    /// 4. Executes `rule.implementation(repository_ctx)` in Starlark
    ///
    /// # Arguments
    ///
    /// * `ctx` - DICE computation context for loading modules
    /// * `invocation` - The repository rule invocation (name, rule_name, attrs)
    /// * `rule_bzl_path` - The .bzl file path (e.g. "@@rules_oci//oci:pull.bzl")
    /// * `rule_name` - The rule name within that file (e.g. "oci_pull")
    /// * `working_dir` - Directory where the repository should be created
    async fn execute_rule(
        &self,
        ctx: &mut DiceComputations<'_>,
        invocation: &RepositoryInvocation,
        rule_bzl_path: &str,
        rule_name: &str,
        working_dir: &Path,
    ) -> kuro_error::Result<()>;
}

/// Late binding for the Starlark repository rule executor.
///
/// Initialized by `kuro_interpreter_for_build::init_late_bindings()`.
pub static STARLARK_REPO_RULE_EXECUTOR_IMPL: LateBinding<
    &'static dyn StarlarkRepoRuleExecutorImpl,
> = LateBinding::new("STARLARK_REPO_RULE_EXECUTOR_IMPL");
