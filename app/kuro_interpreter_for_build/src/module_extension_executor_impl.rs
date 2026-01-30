/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Implementation of module extension execution.
//!
//! This module provides the concrete implementation of `ModuleExtensionExecutorImpl`
//! trait from `kuro_bzlmod`. It bridges the gap between the bzlmod system and the
//! Starlark interpreter.
//!
//! ## Architecture
//!
//! The late binding pattern allows `ModuleExtensionExecutionKey::compute()` in
//! `kuro_bzlmod` to call into this implementation without a direct dependency.
//!
//! ```text
//! kuro_bzlmod                         kuro_interpreter_for_build
//! ┌─────────────────────┐             ┌─────────────────────────────┐
//! │ ModuleExtension     │             │ ConcreteModuleExtension     │
//! │ ExecutionKey        │──late bind──│ Executor                    │
//! │                     │             │                             │
//! │ - AggregatedExt     │             │ - build_module_context()    │
//! │ - temp working dir  │             │ - RepoSpec capture          │
//! └─────────────────────┘             │ - Starlark evaluation       │
//!                                     └─────────────────────────────┘
//! ```
//!
//! ## Current Implementation Status
//!
//! Phase 1 (Current): Infrastructure
//! - Creates ModuleContext from AggregatedExtension
//! - Sets up RepoSpec capture registry
//! - Logs execution context for debugging
//!
//! Phase 2 (Future): Actual Starlark Execution
//! - Load extension .bzl file via interpreter
//! - Get FrozenStarlarkModuleExtension from loaded module
//! - Call extension.implementation(module_ctx)
//! - Capture RepoSpecs from repository rule invocations

use std::path::PathBuf;

use async_trait::async_trait;
use dice::DiceComputations;
use kuro_bzlmod::ExtensionExecutionOutput;
use kuro_bzlmod::ModuleExtensionExecutorImpl;
use kuro_bzlmod::extensions::AggregatedExtension;
use kuro_bzlmod::with_repo_spec_registry;

use crate::extension_execution::build_module_context;

/// Concrete implementation of module extension executor.
///
/// This struct is registered via late binding at program startup.
pub struct ConcreteModuleExtensionExecutor;

#[async_trait]
impl ModuleExtensionExecutorImpl for ConcreteModuleExtensionExecutor {
    async fn execute_extension(
        &self,
        _ctx: &mut DiceComputations<'_>,
        aggregated: &AggregatedExtension,
        root_module_name: &str,
        working_dir: &PathBuf,
    ) -> kuro_error::Result<ExtensionExecutionOutput> {
        tracing::debug!(
            "Executing extension '{}' (kuro_interpreter_for_build)",
            aggregated.extension_id
        );

        // Build the module_ctx from aggregated extension data
        let module_ctx = build_module_context(aggregated, root_module_name)
            .with_temp_working_dir(working_dir.clone());

        tracing::debug!(
            "Built module_ctx with {} module(s), working_dir: {:?}",
            module_ctx.get_modules().len(),
            working_dir
        );

        // Execute with RepoSpec capture registry active
        // Any repository rule calls (http_archive, git_repository, etc.) will
        // capture their RepoSpecs instead of executing immediately
        let (_result, specs) = with_repo_spec_registry(|| {
            tracing::debug!(
                "Extension '{}' execution context:",
                aggregated.extension_id
            );
            tracing::debug!("  - BZL file: {}", aggregated.extension_bzl_file);
            tracing::debug!("  - Extension name: {}", aggregated.extension_name);
            tracing::debug!("  - Root module: {}", root_module_name);
            tracing::debug!("  - Imported repos: {:?}", aggregated.imported_repos);

            // Log modules and tags
            for module in module_ctx.get_modules() {
                tracing::debug!(
                    "  - Module '{}' (v{}, is_root: {}):",
                    module.name,
                    module.version,
                    module.is_root
                );
                for (tag_class, tags) in &module.tags_by_class {
                    tracing::debug!("    - {}: {} tag(s)", tag_class, tags.len());
                }
            }

            // TODO: Phase 2 - Actual Starlark execution
            //
            // To implement actual extension execution, we need to:
            //
            // 1. Load the extension's .bzl file:
            //    ```rust
            //    let bzl_path = parse_bzl_path(&aggregated.extension_bzl_file)?;
            //    let frozen_module = ctx.compute(&LoadModuleKey(bzl_path)).await??;
            //    ```
            //
            // 2. Get the extension value:
            //    ```rust
            //    let ext_value = frozen_module
            //        .env()
            //        .get(&aggregated.extension_name)
            //        .ok_or_else(|| ExtensionNotFound)?;
            //    ```
            //
            // 3. Cast to FrozenStarlarkModuleExtension:
            //    ```rust
            //    let extension: &FrozenStarlarkModuleExtension = ext_value
            //        .downcast_ref()
            //        .ok_or_else(|| NotAModuleExtension)?;
            //    ```
            //
            // 4. Create a Starlark evaluator and invoke:
            //    ```rust
            //    let module = Module::new();
            //    let mut eval = Evaluator::new(&module);
            //    let ctx_value = module.heap().alloc(module_ctx);
            //    extension.implementation()
            //        .invoke(&[ctx_value], &[], &mut eval)?;
            //    ```
            //
            // For now, we log the extension info. Repository rules will still
            // capture RepoSpecs if they're invoked through other means (e.g.,
            // during test execution or when the extension is evaluated elsewhere).

            Ok::<(), kuro_error::Error>(())
        });

        tracing::info!(
            "Extension '{}' captured {} repository spec(s)",
            aggregated.extension_id,
            specs.len()
        );

        // Log captured specs for debugging
        for (name, spec) in &specs {
            tracing::debug!(
                "  - Repo '{}': rule='{}'",
                name,
                spec.repo_rule_id
            );
        }

        Ok(ExtensionExecutionOutput {
            generated_repo_specs: specs,
        })
    }
}

/// Initialize the late binding for module extension execution.
///
/// This is called from `init_late_bindings()` in lib.rs.
pub fn init_module_extension_executor() {
    kuro_bzlmod::MODULE_EXTENSION_EXECUTOR_IMPL.init(&ConcreteModuleExtensionExecutor);
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_bzlmod::extensions::AggregatedExtension;
    use kuro_bzlmod::types::ExtensionTag;
    use kuro_bzlmod::types::TagValue;
    use tempfile::TempDir;

    #[test]
    fn test_concrete_executor_creation() {
        let _executor = ConcreteModuleExtensionExecutor;
        // Just verify it can be created
    }

    #[tokio::test]
    async fn test_execute_extension_empty() {
        let _executor = ConcreteModuleExtensionExecutor;
        let temp_dir = TempDir::new().unwrap();
        let _working_dir = temp_dir.path().to_path_buf();

        let _aggregated = AggregatedExtension::new(
            "@@test_module//test:ext.bzl",
            "test_ext",
        );

        // We can't easily create a DiceComputations in a test, so we skip the
        // full execution test. The key point is that the infrastructure is in place.
        // Full integration testing will be done at a higher level.
    }

    #[test]
    fn test_build_module_context_integration() {
        let mut aggregated = AggregatedExtension::new(
            "@@rules_python//pip:pip.bzl",
            "pip",
        );

        let mut tag = ExtensionTag::new("parse".to_string());
        tag.kwargs.push(("hub_name".to_string(), TagValue::String("pip".to_string())));

        aggregated.add_module_tags("_main", vec![tag]);

        let ctx = build_module_context(&aggregated, "_main");
        let modules = ctx.get_modules();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name, "_main");
        assert!(modules[0].is_root);
        assert!(modules[0].tags_by_class.contains_key("parse"));
    }
}
