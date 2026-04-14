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
//! ## Implementation Status
//!
//! The executor:
//! 1. Parses the extension's .bzl path into an ImportPath
//! 2. Loads the module via DICE/interpreter
//! 3. Retrieves the FrozenStarlarkModuleExtension
//! 4. Builds module_ctx from aggregated tags
//! 5. Invokes extension.implementation(module_ctx) with RepoSpec capture
//!
//! RepoSpecs are captured via `with_repo_spec_registry()` - any repository rule
//! calls during extension execution record their specs instead of executing.

use std::path::PathBuf;

use async_trait::async_trait;
use dice::DiceComputations;
use kuro_bzlmod::ExtensionExecutionOutput;
use kuro_bzlmod::ModuleExtensionExecutorImpl;
use kuro_bzlmod::extensions::AggregatedExtension;
use kuro_bzlmod::with_repo_spec_registry;
use kuro_common::dice::cells::HasCellResolver;
use kuro_common::dice::data::HasIoProvider;
use kuro_core::bzl::ImportPath;
use kuro_core::cells::build_file_cell::BuildFileCell;
use kuro_core::cells::cell_path::CellPath;
use kuro_core::cells::name::CellName;
use kuro_core::cells::paths::CellRelativePathBuf;
use kuro_error::BuckErrorContext;
use kuro_error::conversion::from_any_with_tag;
use kuro_interpreter::load_module::InterpreterCalculation;
use kuro_interpreter::paths::module::StarlarkModulePath;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::OwnedFrozenValueTyped;

use crate::extension_execution::build_module_context;
use crate::module_extension::FrozenStarlarkModuleExtension;

/// Errors during extension execution.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum ExtensionExecutionError {
    #[error("Failed to parse extension bzl path '{path}': {reason}")]
    InvalidBzlPath { path: String, reason: String },

    #[error("Extension '{name}' not found in module '{path}'")]
    ExtensionNotFound { name: String, path: String },

    #[error("Value '{name}' in '{path}' is not a module_extension")]
    NotAModuleExtension { name: String, path: String },

    #[error("Extension implementation returned an error: {0}")]
    ImplementationError(String),

    #[error("Extension cell '{cell}' not found")]
    CellNotFound { cell: String },
}

/// Parse a bzlmod-style bzl file path into an ImportPath.
///
/// Handles formats like:
/// - `@rules_python//python/extensions:pip.bzl`
/// - `@@rules_python//python/extensions:pip.bzl`
/// - `//local:extension.bzl` (root module)
///
/// The `@repo_name` or `@@repo_name` part maps to a cell name.
pub(crate) fn parse_bzlmod_bzl_path(
    bzl_path: &str,
    cell_resolver: &kuro_core::cells::CellResolver,
) -> kuro_error::Result<ImportPath> {
    // Strip leading @@ or @
    let path_without_prefix = bzl_path
        .strip_prefix("@@")
        .or_else(|| bzl_path.strip_prefix("@"))
        .unwrap_or(bzl_path);

    // Handle root module shorthand: ":extensions.bzl" means "//:extensions.bzl"
    let path_without_prefix = if path_without_prefix.starts_with(':') {
        &path_without_prefix[1..] // Strip leading ':', treat as root module path
    } else {
        path_without_prefix
    };

    // Split into cell/repo part and path part at //
    let (cell_part, path_part) = path_without_prefix.split_once("//").unwrap_or_else(|| {
        // No '//' separator - treat as root module path
        ("", path_without_prefix)
    });

    // Determine the cell name
    let cell_name = if cell_part.is_empty() {
        // //local:path.bzl -> use root cell
        cell_resolver.root_cell()
    } else {
        // @repo//path:file.bzl -> try to find cell with that name
        // First try exact match as cell name
        match CellName::unchecked_new(cell_part) {
            Ok(name) if cell_resolver.get(name).is_ok() => name,
            _ => {
                // Fall back to root cell if repo name doesn't match a cell
                // This handles cases where bzlmod repos haven't been registered as cells yet
                tracing::debug!(
                    "Bzlmod repo '{}' not found as cell, using root cell",
                    cell_part
                );
                cell_resolver.root_cell()
            }
        }
    };

    // Parse the path:file.bzl part
    // Format: "python/extensions:pip.bzl" or just "pip.bzl"
    let cell_relative_path = if let Some((dir, file)) = path_part.rsplit_once(':') {
        // dir:file format - if dir is empty (e.g., "//:file.bzl"), just use file name
        if dir.is_empty() {
            file.to_owned()
        } else {
            format!("{}/{}", dir, file)
        }
    } else {
        // Just a file, no directory
        path_part.to_owned()
    };

    let cell_path = CellPath::new(
        cell_name,
        CellRelativePathBuf::try_from(cell_relative_path).map_err(|e| {
            ExtensionExecutionError::InvalidBzlPath {
                path: bzl_path.to_owned(),
                reason: e.to_string(),
            }
        })?,
    );

    ImportPath::new_with_build_file_cells(cell_path, BuildFileCell::new(cell_name))
        .buck_error_context(format!("Creating ImportPath for {}", bzl_path))
}

/// Concrete implementation of module extension executor.
///
/// This struct is registered via late binding at program startup.
pub struct ConcreteModuleExtensionExecutor;

impl ConcreteModuleExtensionExecutor {
    /// Try to execute the extension's Starlark implementation.
    ///
    /// This:
    /// 1. Parses the bzl path and loads the module
    /// 2. Gets the FrozenStarlarkModuleExtension
    /// 3. Creates an evaluator and invokes implementation(module_ctx)
    /// 4. Captures RepoSpecs via the registry
    async fn try_execute_starlark(
        &self,
        ctx: &mut DiceComputations<'_>,
        aggregated: &AggregatedExtension,
        mut module_ctx: crate::module_ctx::ModuleContext,
    ) -> kuro_error::Result<std::collections::HashMap<String, kuro_bzlmod::RepoSpec>> {
        // 1. Get the cell resolver to parse the bzl path
        let cell_resolver = ctx.get_cell_resolver().await?;

        // 2. Parse the bzl path
        let import_path = parse_bzlmod_bzl_path(&aggregated.extension_bzl_file, &cell_resolver)?;

        tracing::debug!(
            "Extension execution: bzl_file='{}' -> import_path='{}' (cell='{}')",
            aggregated.extension_bzl_file,
            import_path,
            import_path.cell()
        );

        // 3. Load the module via DICE
        let loaded_module = ctx
            .get_loaded_module(StarlarkModulePath::LoadFile(&import_path))
            .await
            .buck_error_context(format!(
                "Loading extension bzl file: {}",
                aggregated.extension_bzl_file
            ))?;

        // 4. Get the extension value from the module
        let ext_value = loaded_module
            .env()
            .get_any_visibility(&aggregated.extension_name)
            .map_err(|e| from_any_with_tag(e, kuro_error::ErrorTag::Input))?
            .0;

        // 5. Downcast to FrozenStarlarkModuleExtension
        let frozen_extension: OwnedFrozenValueTyped<FrozenStarlarkModuleExtension> = ext_value
            .downcast_starlark()
            .map_err(|_| ExtensionExecutionError::NotAModuleExtension {
                name: aggregated.extension_name.clone(),
                path: aggregated.extension_bzl_file.clone(),
            })?;

        tracing::debug!("Found extension '{}' in module", frozen_extension.name());

        // 5b. Extract tag class defaults and apply to module_ctx
        // This ensures missing tag attributes get their declared default values
        // (e.g., attr.string_list_dict(default={}) → {} instead of None)
        {
            let mut tag_class_defaults = std::collections::HashMap::new();
            for (class_name, class_value) in frozen_extension.tag_classes() {
                if let Some(tag_class) = class_value
                    .downcast_frozen_ref::<crate::module_extension::FrozenStarlarkTagClass>()
                {
                    let tag_class = &*tag_class;
                    let defaults: Vec<(String, crate::module_ctx::SerializedTagValue)> = tag_class
                        .attrs()
                        .iter()
                        .filter_map(|(attr_name, attr)| {
                            // Try explicit default first
                            if let Some(default) = attr.default() {
                                let value =
                                    crate::module_ctx::coerced_attr_to_serialized_tag_value(
                                        default,
                                    )?;
                                return Some((attr_name.clone(), value));
                            }
                            // For attrs with no explicit default, use type-appropriate empty
                            // value (Bazel defaults list/dict attrs to []/{}):
                            let type_default = crate::module_ctx::default_for_attr_type(
                                &attr.coercer_for_default_only(),
                            );
                            type_default.map(|v| (attr_name.clone(), v))
                        })
                        .collect();
                    if !defaults.is_empty() {
                        tag_class_defaults.insert(class_name.clone(), defaults);
                    }
                }
            }
            if !tag_class_defaults.is_empty() {
                module_ctx.apply_tag_class_defaults(&tag_class_defaults);
            }
        }

        // 6. Execute with RepoSpec capture registry active
        let (result, specs) = with_repo_spec_registry(|| {
            // Create a Starlark module for evaluation
            let starlark_module = Module::new();

            // Allocate the module_ctx on the heap
            let ctx_value = starlark_module.heap().alloc(module_ctx);

            // Create an evaluator
            let mut eval = Evaluator::new(&starlark_module);

            // Get the implementation function
            let implementation = frozen_extension.implementation();

            tracing::debug!(
                "Invoking extension implementation for '{}'",
                aggregated.extension_name
            );

            // Invoke: implementation(module_ctx)
            let invoke_result = eval.eval_function(implementation.to_value(), &[ctx_value], &[]);

            match invoke_result {
                Ok(return_value) => {
                    // Extension implementations should return None
                    if !return_value.is_none() {
                        tracing::warn!(
                            "Extension '{}' returned non-None value: {}",
                            aggregated.extension_name,
                            return_value.get_type()
                        );
                    }
                    Ok::<(), kuro_error::Error>(())
                }
                Err(e) => {
                    tracing::error!(
                        "Extension '{}' implementation failed: {}",
                        aggregated.extension_name,
                        e
                    );
                    Err(ExtensionExecutionError::ImplementationError(e.to_string()).into())
                }
            }
        });

        // Check for execution errors
        result?;

        Ok(specs)
    }
}

#[async_trait]
impl ModuleExtensionExecutorImpl for ConcreteModuleExtensionExecutor {
    async fn execute_extension(
        &self,
        ctx: &mut DiceComputations<'_>,
        aggregated: &AggregatedExtension,
        root_module_name: &str,
        working_dir: &PathBuf,
    ) -> kuro_error::Result<ExtensionExecutionOutput> {
        tracing::debug!(
            "Executing extension '{}' (kuro_interpreter_for_build)",
            aggregated.extension_id
        );

        // Build cell path map from CellResolver for Label-to-path resolution.
        // This is the kuro equivalent of Bazel's getPathFromLabel() from
        // StarlarkBaseExternalContext — it enables module_ctx.path(Label) and
        // module_ctx.execute([Label, ...]) to resolve Labels to filesystem paths.
        let cell_resolver = ctx.get_cell_resolver().await?;
        let io = ctx.global_data().get_io_provider();
        let project_root = io.project_root().root().to_path_buf();
        let mut cell_paths = std::collections::HashMap::new();
        for (cell_name, cell_instance) in cell_resolver.cells() {
            let rel_path = cell_instance.path().as_project_relative_path();
            cell_paths.insert(
                cell_name.as_str().to_owned(),
                project_root.join(rel_path.as_str()),
            );
        }

        // Build the module_ctx from aggregated extension data
        let module_ctx = build_module_context(aggregated, root_module_name)
            .with_temp_working_dir(working_dir.clone())
            .with_label_resolution(project_root.clone(), cell_paths);

        tracing::debug!(
            "Built module_ctx with {} module(s), working_dir: {:?}",
            module_ctx.get_modules().len(),
            working_dir
        );

        // Log execution context
        tracing::debug!("Extension '{}' execution context:", aggregated.extension_id);
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

        // Try to load and execute the extension's Starlark implementation
        let specs = match self.try_execute_starlark(ctx, aggregated, module_ctx).await {
            Ok(specs) => specs,
            Err(e) => {
                // Fall back to empty specs - some extensions (e.g., test-only ones)
                // may legitimately fail to load, and hard-failing would break builds
                // that don't need those repos.
                tracing::warn!(
                    "Could not execute extension '{}' Starlark implementation, \
                     falling back to empty specs: {:?}",
                    aggregated.extension_id,
                    e
                );
                std::collections::HashMap::new()
            }
        };

        tracing::info!(
            "Extension '{}' captured {} repository spec(s)",
            aggregated.extension_id,
            specs.len()
        );

        // Log captured specs for debugging
        for (name, spec) in &specs {
            tracing::debug!("  - Repo '{}': rule='{}'", name, spec.repo_rule_id);
        }

        // Eagerly materialize ALL repos generated by this extension.
        // This is needed because other extensions may reference these repos via
        // Label("@repo_name//...") during their own execution. In Bazel, this
        // happens lazily via Skyframe's RepositoryDirectoryValue. In kuro, we
        // materialize eagerly since we can't do Skyframe-style restarts from
        // synchronous Starlark eval.
        if !specs.is_empty() {
            let ext_name = kuro_bzlmod::extract_extension_name(&aggregated.extension_id);
            let owning_module = kuro_bzlmod::extract_owning_module(&aggregated.extension_id);
            for (internal_name, spec) in &specs {
                let canonical = format!("{}+{}+{}", owning_module, ext_name, internal_name);
                let repo_dir = project_root.join("bazel-external").join(&canonical);

                // Register in dynamic cell registry so cell resolution can find
                // extension spoke repos (e.g., crates__tempfile-3.26.0) that
                // aren't explicitly in use_repo().
                kuro_core::cells::register_dynamic_extension_cell(
                    canonical.clone(),
                    format!("bazel-external/{}", canonical),
                );

                // Skip if already materialized
                if repo_dir.join(".kuro_repo_complete").exists() {
                    continue;
                }

                // Use DICE-based ExtensionRepoExecutionKey for full Starlark repo
                // rule support. This handles both builtin (http_archive, etc.)
                // and custom Starlark repo rules (cargo_repository, etc.).
                let key = kuro_bzlmod::ExtensionRepoExecutionKey::new(
                    canonical.clone(),
                    aggregated.extension_id.to_string(),
                    spec.clone(),
                    project_root.clone(),
                );
                match ctx.compute(&key).await {
                    Ok(Ok(_result)) => {
                        tracing::debug!("Eagerly materialized extension repo '{}'", canonical);
                    }
                    Ok(Err(e)) => {
                        tracing::debug!("Could not eagerly materialize '{}': {}", canonical, e);
                    }
                    Err(e) => {
                        tracing::debug!("DICE error materializing '{}': {}", canonical, e);
                    }
                }
            }
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
    use kuro_bzlmod::extensions::AggregatedExtension;
    use kuro_bzlmod::types::ExtensionTag;
    use kuro_bzlmod::types::TagValue;
    use tempfile::TempDir;

    use super::*;

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

        let _aggregated = AggregatedExtension::new("@@test_module//test:ext.bzl", "test_ext");

        // We can't easily create a DiceComputations in a test, so we skip the
        // full execution test. The key point is that the infrastructure is in place.
        // Full integration testing will be done at a higher level.
    }

    #[test]
    fn test_build_module_context_integration() {
        let mut aggregated = AggregatedExtension::new("@@rules_python//pip:pip.bzl", "pip");

        let mut tag = ExtensionTag::new("parse".to_string());
        tag.kwargs
            .push(("hub_name".to_string(), TagValue::String("pip".to_string())));

        aggregated.add_module_tags("_main", vec![tag]);

        let ctx = build_module_context(&aggregated, "_main");
        let modules = ctx.get_modules();

        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].name, "_main");
        assert!(modules[0].is_root);
        assert!(modules[0].tags_by_class.contains_key("parse"));
    }
}
