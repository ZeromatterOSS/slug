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
//! trait from `slug_bzlmod`. It bridges the gap between the bzlmod system and the
//! Starlark interpreter.
//!
//! ## Architecture
//!
//! The late binding pattern allows `ModuleExtensionExecutionKey::compute()` in
//! `slug_bzlmod` to call into this implementation without a direct dependency.
//!
//! ```text
//! slug_bzlmod                         slug_interpreter_for_build
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

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use slug_bzlmod::AggregatedExtension;
use slug_bzlmod::ExtensionExecutionOutput;
use slug_bzlmod::ModuleExtensionExecutorImpl;
use slug_bzlmod::WorkspaceId;
use slug_bzlmod::compute_bzl_transitive_digest_from_file_states;
use slug_bzlmod::with_repo_spec_registry;
use slug_common::dice::cells::HasCellResolver;
use slug_common::dice::data::HasIoProvider;
use slug_common::file_ops::dice::DiceFileComputations;
use slug_common::file_ops::error::FileReadError;
use slug_core::bzl::ImportPath;
use slug_core::cells::build_file_cell::BuildFileCell;
use slug_core::cells::cell_path::CellPath;
use slug_core::cells::name::CellName;
use slug_core::cells::paths::CellRelativePathBuf;
use slug_error::BuckErrorContext;
use slug_error::conversion::from_any_with_tag;
use slug_interpreter::file_loader::LoadedModule;
use slug_interpreter::load_module::InterpreterCalculation;
use slug_interpreter::paths::module::OwnedStarlarkModulePath;
use slug_interpreter::paths::module::StarlarkModulePath;
use slug_interpreter::paths::path::OwnedStarlarkPath;
use slug_interpreter::paths::path::StarlarkPath;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::OwnedFrozenValueTyped;
use starlark::values::ValueLike;

use crate::extension_execution::build_module_context;
use crate::interpreter::dice_calculation_delegate::HasCalculationDelegate;
use crate::module_ctx::StarlarkModuleExtensionMetadata;
use crate::module_extension::FrozenStarlarkModuleExtension;

/// Errors during extension execution.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
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

#[derive(Clone, Debug, Display, PartialEq, Eq, Hash, Allocative)]
#[display(
    "ModuleExtensionLoadedModuleKey({}, {}, {})",
    workspace_id.stable_hash,
    extension_bzl_file,
    bzl_transitive_digest
)]
struct ModuleExtensionLoadedModuleKey {
    workspace_id: WorkspaceId,
    extension_bzl_file: Arc<str>,
    bzl_transitive_digest: Arc<str>,
}

#[derive(Debug, Allocative)]
struct ModuleExtensionLoadedModuleValue {
    loaded_module: LoadedModule,
    bzl_transitive_digest: Arc<str>,
}

#[async_trait]
impl Key for ModuleExtensionLoadedModuleKey {
    type Value = slug_error::Result<Arc<ModuleExtensionLoadedModuleValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let cell_resolver = ctx.get_cell_resolver().await?;
        let import_path = parse_bzlmod_bzl_path(&self.extension_bzl_file, &cell_resolver)?;
        let loaded_module = ctx
            .get_loaded_module(StarlarkModulePath::LoadFile(&import_path))
            .await
            .buck_error_context(format!(
                "Loading extension bzl file: {}",
                self.extension_bzl_file
            ))?;

        Ok(Arc::new(ModuleExtensionLoadedModuleValue {
            loaded_module,
            bzl_transitive_digest: self.bzl_transitive_digest.clone(),
        }))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.bzl_transitive_digest == y.bzl_transitive_digest,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
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
    cell_resolver: &slug_core::cells::CellResolver,
) -> slug_error::Result<ImportPath> {
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
        // @repo//path:file.bzl -> try to find cell with that name.
        //
        // Bazel canonical repository labels in lockfiles and captured repo
        // specs can look like `@@rules_nodejs+//nodejs:repositories.bzl`.
        // Slug's source module cells are registered under the apparent module
        // name (`rules_nodejs`), while extension repos keep the full
        // `module+extension+repo` name. Try the canonical spelling first, then
        // the apparent module prefix before falling back.
        let canonical_candidate;
        let mut candidates = vec![cell_part];
        if let Some(stripped) = cell_part.strip_suffix('+') {
            candidates.push(stripped);
        } else if !cell_part.contains('+') {
            canonical_candidate = format!("{cell_part}+");
            candidates.push(&canonical_candidate);
        }
        if let Some((apparent, _)) = cell_part.split_once('+') {
            candidates.push(apparent);
        }

        candidates
            .into_iter()
            .find_map(|candidate| {
                CellName::unchecked_new(candidate)
                    .ok()
                    .filter(|name| cell_resolver.get(*name).is_ok())
            })
            .unwrap_or_else(|| {
                // Fall back to root cell if repo name doesn't match a cell
                // This handles cases where bzlmod repos haven't been registered as cells yet
                tracing::debug!(
                    "Bzlmod repo '{}' not found as cell, using root cell",
                    cell_part
                );
                cell_resolver.root_cell()
            })
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

const MISSING_FILE_DIGEST_ERROR: &str = "No such file or directory (os error 2)";

fn record_declared_extension_environ(
    module_ctx: &crate::module_ctx::ModuleContext,
    environ: &[String],
) -> slug_error::Result<()> {
    let mut env_names: Vec<&String> = environ.iter().collect();
    env_names.sort();
    env_names.dedup();
    for env in env_names {
        module_ctx.record_env_input(env).map_err(|e| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Failed to record module extension declared environ '{}': {}",
                env,
                e
            )
        })?;
    }
    Ok(())
}

async fn read_loaded_bzl_file_for_digest(
    ctx: &mut DiceComputations<'_>,
    cell_path: &CellPath,
) -> Result<String, String> {
    match DiceFileComputations::read_file(ctx, cell_path.as_ref()).await {
        Ok(content) => Ok(content),
        Err(FileReadError::NotFound(_)) => Err(MISSING_FILE_DIGEST_ERROR.to_owned()),
        Err(error) => Err(error.without_package_context_information().to_string()),
    }
}

impl ConcreteModuleExtensionExecutor {
    async fn try_loaded_bzl_transitive_digest(
        &self,
        ctx: &mut DiceComputations<'_>,
        extension_id: &str,
        aggregated: &AggregatedExtension,
        allow_missing_loads: bool,
    ) -> slug_error::Result<String> {
        let cell_resolver = ctx.get_cell_resolver().await?;
        let import_path = parse_bzlmod_bzl_path(&aggregated.extension_bzl_file, &cell_resolver)?;
        let root_path = OwnedStarlarkModulePath::new(StarlarkModulePath::LoadFile(&import_path));

        let mut queue = VecDeque::from([root_path]);
        let mut seen = BTreeSet::new();
        let mut file_states = BTreeMap::new();
        while let Some(module_path) = queue.pop_front() {
            if !seen.insert(module_path.to_string()) {
                continue;
            }
            // The interpreter autoloads Slug's Bazel-compat builtins. Those
            // are not Bazel module-extension implementation inputs.
            if module_path.path().cell().as_str() == "slug_builtins" {
                continue;
            }

            let StarlarkModulePath::LoadFile(import_path) = module_path.borrow() else {
                continue;
            };
            let cell_path = import_path.path();
            let cell = cell_resolver.get(cell_path.cell())?;
            let project_relative = cell.path().join(cell_path.path());

            let content = read_loaded_bzl_file_for_digest(ctx, cell_path).await;
            let content = match content {
                Ok(content) => content,
                Err(error) => {
                    if !allow_missing_loads {
                        let reason = if error.contains("No such file") {
                            format!("File not found: {}", project_relative)
                        } else {
                            error
                        };
                        return Err(slug_error::slug_error!(
                            slug_error::ErrorTag::Input,
                            "Reading loaded extension .bzl file '{}' for transitive digest: {}",
                            project_relative,
                            reason
                        ));
                    }
                    file_states.insert(project_relative.as_str().to_owned(), Err(error));
                    continue;
                }
            };

            {
                let starlark_path = StarlarkPath::LoadFile(import_path);
                let parsed = ctx
                    .get_interpreter_calculator(OwnedStarlarkPath::new(starlark_path))
                    .await?
                    .prepare_eval_with_content(starlark_path, content.clone())?;
                let parsed = parsed.with_buck_error_context(|| {
                    format!(
                        "Parsing loaded extension .bzl file '{}' for transitive digest",
                        project_relative
                    )
                })?;
                for (_, import) in parsed.imports().iter() {
                    if matches!(import.borrow(), StarlarkModulePath::LoadFile(_)) {
                        queue.push_back(import.clone());
                    }
                }
            }
            file_states.insert(project_relative.as_str().to_owned(), Ok(content));
        }

        Ok(compute_bzl_transitive_digest_from_file_states(
            extension_id,
            &file_states,
        ))
    }

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
        workspace_id: WorkspaceId,
        bzl_transitive_digest: Arc<str>,
        mut module_ctx: crate::module_ctx::ModuleContext,
    ) -> slug_error::Result<ExtensionExecutionOutput> {
        // 1. Load the module through a stable extension-specific DICE key.
        // The generic interpreter load key cannot compare Starlark modules for
        // equality, while extension execution already has a strict transitive
        // implementation digest as its semantic input.
        let loaded_module = ctx
            .compute(&ModuleExtensionLoadedModuleKey {
                workspace_id: workspace_id.clone(),
                extension_bzl_file: Arc::from(aggregated.extension_bzl_file.as_str()),
                bzl_transitive_digest,
            })
            .await??
            .loaded_module
            .dupe();

        tracing::debug!(
            "Extension execution: bzl_file='{}' -> module_path='{}'",
            aggregated.extension_bzl_file,
            loaded_module.path()
        );

        // 2. Get the extension value from the module
        let ext_value = loaded_module
            .env()
            .get_any_visibility(&aggregated.extension_name)
            .map_err(|e| from_any_with_tag(e, slug_error::ErrorTag::Input))?
            .0;

        // 3. Downcast to FrozenStarlarkModuleExtension
        let frozen_extension: OwnedFrozenValueTyped<FrozenStarlarkModuleExtension> = ext_value
            .downcast_starlark()
            .map_err(|_| ExtensionExecutionError::NotAModuleExtension {
                name: aggregated.extension_name.clone(),
                path: aggregated.extension_bzl_file.clone(),
            })?;

        tracing::debug!("Found extension '{}' in module", frozen_extension.name());

        // 3b. Extract tag class defaults and apply to module_ctx
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

        // 4. Execute with RepoSpec capture registry active.
        //
        // Plan 36: also stash a thread-local pointer to `ctx` so that
        // `mctx.path(Label)` / `mctx.read(Label)` calls inside the eval
        // can drive lazy materialization of sibling-extension spoke
        // repos via `slug_bzlmod::materialize_spoke_sync`.
        record_declared_extension_environ(&module_ctx, frozen_extension.environ())?;
        let recorded_inputs_ctx = module_ctx.clone();

        let (result, specs) = slug_bzlmod::with_extension_dice(ctx, workspace_id, || {
            with_repo_spec_registry(|| {
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
                let invoke_result =
                    eval.eval_function(implementation.to_value(), &[ctx_value], &[]);

                match invoke_result {
                    Ok(return_value) => {
                        if return_value.is_none() {
                            return Ok::<slug_bzlmod::ModuleExtensionMetadata, slug_error::Error>(
                                Default::default(),
                            );
                        }

                        let Some(metadata) =
                            return_value.downcast_ref::<StarlarkModuleExtensionMetadata>()
                        else {
                            return Err(ExtensionExecutionError::ImplementationError(format!(
                                "module extension implementation must return None or module_ctx.extension_metadata(...), got {}",
                                return_value.get_type()
                            ))
                            .into());
                        };

                        Ok(metadata.metadata().clone())
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
            })
        });

        // Check for execution errors
        let metadata = result?;
        let recorded_inputs = recorded_inputs_ctx.recorded_inputs()?;

        Ok(ExtensionExecutionOutput {
            generated_repo_specs: specs,
            metadata,
            recorded_inputs,
        })
    }
}

#[async_trait]
impl ModuleExtensionExecutorImpl for ConcreteModuleExtensionExecutor {
    async fn extension_bzl_transitive_digest(
        &self,
        ctx: &mut DiceComputations<'_>,
        extension_id: &str,
        aggregated: &AggregatedExtension,
        allow_missing_loads: bool,
    ) -> slug_error::Result<String> {
        self.try_loaded_bzl_transitive_digest(ctx, extension_id, aggregated, allow_missing_loads)
            .await
    }

    async fn execute_extension(
        &self,
        ctx: &mut DiceComputations<'_>,
        aggregated: &AggregatedExtension,
        root_module_name: &str,
        working_dir: &PathBuf,
        prior_facts: serde_json::Value,
        repo_env: Arc<BTreeMap<String, String>>,
        bzl_transitive_digest: Arc<str>,
        workspace_id: WorkspaceId,
    ) -> slug_error::Result<ExtensionExecutionOutput> {
        tracing::debug!(
            "Executing extension '{}' (slug_interpreter_for_build)",
            aggregated.extension_id
        );

        // Build cell path map from CellResolver for Label-to-path resolution.
        // This is the slug equivalent of Bazel's getPathFromLabel() from
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
        for (cell_name, rel_path) in cell_resolver.bzlmod_label_cell_paths() {
            cell_paths
                .entry(cell_name)
                .or_insert_with(|| project_root.join(rel_path));
        }
        let owning_module =
            slug_bzlmod::extract_owning_module(&aggregated.extension_id, root_module_name);
        for (cell_name, rel_path) in
            cell_resolver.bzlmod_label_cell_paths_for_owner(Some(&owning_module))
        {
            cell_paths
                .entry(cell_name)
                .or_insert_with(|| project_root.join(rel_path));
        }

        // Build the module_ctx from aggregated extension data
        let module_ctx = build_module_context(aggregated, root_module_name)
            .with_temp_working_dir(working_dir.clone())
            .with_label_resolution_and_root_cell(
                project_root.clone(),
                cell_paths,
                Some(cell_resolver.root_cell().as_str().to_owned()),
            )
            .with_facts(prior_facts)
            .with_repo_env(repo_env.clone());

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

        let output = self
            .try_execute_starlark(
                ctx,
                aggregated,
                workspace_id,
                bzl_transitive_digest.clone(),
                module_ctx,
            )
            .await
            .buck_error_context(format!(
                "module extension '{}' failed",
                aggregated.extension_id
            ))?;
        let specs = output.generated_repo_specs.clone();

        tracing::info!(
            "Extension '{}' captured {} repository spec(s)",
            aggregated.extension_id,
            specs.len()
        );

        // Log captured specs for debugging
        for (name, spec) in &specs {
            tracing::debug!("  - Repo '{}': rule='{}'", name, spec.repo_rule_id);
        }

        // Register generated spokes up front so label resolution and later lazy
        // materialization can find them. Do not compute ExtensionRepoExecutionKey
        // here: repository materialization has its own DICE inputs, and recording
        // those as dependencies of extension evaluation would make a successful
        // extension eval non-reusable when only materialization marker state needs
        // checking.
        if !specs.is_empty() {
            let ext_name = slug_bzlmod::extract_extension_name(&aggregated.extension_id);
            // Pass `root_module_name` so the root module's declared name (e.g.
            // `llvm-project-overlay`) is canonicalized to `_main`, matching
            // what `pending_repo_cells.rs` registers for the same repo.
            let repo_env_json =
                serde_json::to_string(repo_env.as_ref()).unwrap_or_else(|_| "{}".to_owned());

            for (internal_name, spec) in &specs {
                let canonical = format!("{}+{}+{}", owning_module, ext_name, internal_name);
                let repo_spec_json = serde_json::to_string(spec).buck_error_context(format!(
                    "Serializing repo spec for generated repo '{}'",
                    canonical
                ))?;
                let setup = slug_core::cells::external::ExtensionRepoCellSetup {
                    canonical_name: Arc::from(canonical.as_str()),
                    extension_id: Arc::from(aggregated.extension_id.as_str()),
                    internal_name: Arc::from(internal_name.as_str()),
                    spec_hash: Arc::from(
                        slug_bzlmod::repo_execution_spec_hash(spec, repo_env.as_ref()).as_str(),
                    ),
                    repo_spec_json: Arc::from(repo_spec_json.as_str()),
                    repo_env_json: Arc::from(repo_env_json.as_str()),
                    extension_usages_digest: Arc::from(""),
                    extension_replay_inputs_identity_digest: Arc::from(""),
                    extension_repo_mappings_digest: Arc::from(""),
                    extension_repo_mapping_overrides_digest: Arc::from(""),
                    extension_bzl_transitive_digest: bzl_transitive_digest.clone(),
                    extension_recorded_inputs_json: Arc::from(
                        serde_json::to_string(&output.recorded_inputs)
                            .unwrap_or_else(|_| "[]".to_owned())
                            .as_str(),
                    ),
                    materialized: false,
                };
                cell_resolver.register_bzlmod_runtime_extension_cell(
                    &canonical,
                    &format!("bazel-external/{}", canonical),
                    setup,
                )?;
            }
        }

        Ok(output)
    }
}

/// Initialize the late binding for module extension execution.
///
/// This is called from `init_late_bindings()` in lib.rs.
pub fn init_module_extension_executor() {
    slug_bzlmod::MODULE_EXTENSION_EXECUTOR_IMPL.init(&ConcreteModuleExtensionExecutor);
}

#[cfg(test)]
mod tests {
    use slug_bzlmod::AggregatedExtension;
    use slug_bzlmod::ExtensionTag;
    use slug_bzlmod::TagValue;
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

    #[test]
    fn test_declared_extension_environ_records_inputs() {
        let mut repo_env = BTreeMap::new();
        repo_env.insert("PLAN61_DECLARED_ENV".to_owned(), "from-context".to_owned());
        let ctx = crate::module_ctx::ModuleContext::empty().with_repo_env(Arc::new(repo_env));

        record_declared_extension_environ(
            &ctx,
            &[
                "PLAN61_DECLARED_MISSING".to_owned(),
                "PLAN61_DECLARED_ENV".to_owned(),
                "PLAN61_DECLARED_ENV".to_owned(),
            ],
        )
        .unwrap();

        assert_eq!(
            ctx.recorded_inputs().unwrap(),
            vec![
                "ENV:PLAN61_DECLARED_ENV from-context".to_owned(),
                "ENV:PLAN61_DECLARED_MISSING \\0".to_owned(),
            ]
        );
    }
}
