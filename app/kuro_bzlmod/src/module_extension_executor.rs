/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Late binding trait for module extension execution.
//!
//! This module provides the interface for executing module extensions. The actual
//! implementation lives in `kuro_interpreter_for_build` which has access to the
//! Starlark interpreter and can call `build_module_context()`.
//!
//! ## Why Late Binding?
//!
//! The crate dependency direction is:
//!   `kuro_interpreter_for_build` depends on `kuro_bzlmod`
//!
//! But extension execution needs to:
//!   1. Use `AggregatedExtension` from `kuro_bzlmod`
//!   2. Build `ModuleContext` via `build_module_context()` from `kuro_interpreter_for_build`
//!   3. Execute Starlark code (requires `starlark` crate)
//!
//! The late binding pattern allows `ModuleExtensionExecutionKey::compute()` in
//! `kuro_bzlmod` to call into `kuro_interpreter_for_build` without a direct dependency.

use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use dice::DiceComputations;
use kuro_util::late_binding::LateBinding;

use crate::extensions::AggregatedExtension;
use crate::repo_spec::RepoSpec;

/// Result of extension execution.
///
/// This is a simplified result type that contains just the captured RepoSpecs.
/// The full `ModuleExtensionResult` is built by the caller.
#[derive(Debug, Clone)]
pub struct ExtensionExecutionOutput {
    /// Captured repository specifications (NOT materialized).
    /// Keys are internal names (e.g., "numpy"), values are RepoSpecs.
    ///
    /// `FxHashMap` so that iteration order is stable across invocations
    /// (Plan 21.2 — fixes CellResolver churn).
    pub generated_repo_specs: fxhash::FxHashMap<String, RepoSpec>,
}

/// Trait for module extension execution.
///
/// Implementations provide the actual Starlark execution logic. The implementation
/// lives in `kuro_interpreter_for_build` where it can access:
/// - The Starlark interpreter
/// - `build_module_context()` to create `module_ctx`
/// - The extension's .bzl file loading infrastructure
#[async_trait]
pub trait ModuleExtensionExecutorImpl: Send + Sync + 'static {
    /// Execute a module extension and return the captured RepoSpecs.
    ///
    /// This method:
    /// 1. Loads the extension's .bzl file
    /// 2. Builds `module_ctx` from the aggregated extension data
    /// 3. Sets the working directory on `module_ctx`
    /// 4. Executes `extension.implementation(module_ctx)`
    /// 5. Captures and returns any RepoSpecs created by repository rule calls
    ///
    /// # Arguments
    ///
    /// * `ctx` - DICE computation context for accessing other DICE keys
    /// * `aggregated` - Aggregated extension data from all modules
    /// * `root_module_name` - Name of the root module (for `module_ctx.modules` ordering)
    /// * `working_dir` - Temporary working directory for `module_ctx` I/O operations
    ///
    /// # Returns
    ///
    /// On success, returns `ExtensionExecutionOutput` with the captured RepoSpecs.
    /// The caller is responsible for cleaning up the working directory.
    async fn execute_extension(
        &self,
        ctx: &mut DiceComputations<'_>,
        aggregated: &AggregatedExtension,
        root_module_name: &str,
        working_dir: &PathBuf,
    ) -> kuro_error::Result<ExtensionExecutionOutput>;
}

/// Late binding for the module extension executor.
///
/// Initialized by `kuro_interpreter_for_build::init_late_bindings()`.
///
/// # Example Usage
///
/// ```ignore
/// let executor = MODULE_EXTENSION_EXECUTOR_IMPL.get()?;
/// let output = executor
///     .execute_extension(ctx, &aggregated, "root", &temp_dir)
///     .await?;
/// ```
pub static MODULE_EXTENSION_EXECUTOR_IMPL: LateBinding<&'static dyn ModuleExtensionExecutorImpl> =
    LateBinding::new("MODULE_EXTENSION_EXECUTOR_IMPL");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_execution_output() {
        let mut specs = HashMap::new();
        specs.insert(
            "test_repo".to_owned(),
            RepoSpec::new("@@bazel_tools//repo:http.bzl%http_archive".to_owned()),
        );

        let output = ExtensionExecutionOutput {
            generated_repo_specs: specs,
        };

        assert_eq!(output.generated_repo_specs.len(), 1);
        assert!(output.generated_repo_specs.contains_key("test_repo"));
    }
}
