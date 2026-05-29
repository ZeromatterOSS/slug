/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Concrete implementation of Starlark repository rule execution.
//!
//! This module provides the implementation of `StarlarkRepoRuleExecutorImpl`
//! that bridges the gap between the bzlmod system and the Starlark interpreter.
//!
//! ## Architecture
//!
//! This follows the same late-binding pattern as `module_extension_executor_impl.rs`:
//!
//! ```text
//! slug_bzlmod                             slug_interpreter_for_build
//! ┌─────────────────────────┐             ┌──────────────────────────────────┐
//! │ ExtensionRepoExecution  │             │ ConcreteStarlarkRepoRule         │
//! │ Key                     │──late bind──│ Executor                         │
//! │                         │             │                                  │
//! │ - RepositoryInvocation  │             │ - parse_bzlmod_bzl_path()        │
//! │ - rule_source           │             │ - load .bzl via DICE             │
//! │ - working_dir           │             │ - create RepositoryContext       │
//! └─────────────────────────┘             │ - call rule.implementation(ctx)  │
//!                                         └──────────────────────────────────┘
//! ```

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use dice::DiceComputations;
use slug_bzlmod::RepositoryInvocation;
use slug_bzlmod::StarlarkRepoRuleExecution;
use slug_bzlmod::StarlarkRepoRuleExecutorImpl;
use slug_bzlmod::WorkspaceId;
use slug_common::dice::cells::HasCellResolver;
use slug_common::dice::data::HasIoProvider;
use slug_common::file_ops::dice::DiceFileComputations;
use slug_common::file_ops::error::FileReadErrorContext;
use slug_common::file_ops::metadata::RawPathMetadata;
use slug_core::cells::CellResolver;
use slug_core::cells::cell_path::CellPath;
use slug_core::fs::project::ProjectRoot;
use slug_core::fs::project_rel_path::ProjectRelativePath;
use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
use slug_error::BuckErrorContext;
use slug_error::conversion::from_any_with_tag;
use slug_fs::paths::abs_path::AbsPath;
use slug_fs::paths::forward_rel_path::ForwardRelativePath;
use slug_interpreter::from_freeze::from_freeze_error;
use slug_interpreter::load_module::InterpreterCalculation;
use slug_interpreter::paths::module::StarlarkModulePath;
use starlark::environment::FrozenModule;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::ReturnFileLoader;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::values::OwnedFrozenValueTyped;

use crate::interpreter::globals::register_load_natives;
use crate::module_extension_executor_impl::parse_bzlmod_bzl_path;
use crate::repository_ctx::AttrValue as CtxAttrValue;
use crate::repository_ctx::RepositoryAttr;
use crate::repository_ctx::RepositoryContext;
use crate::repository_ctx::RepositoryWatchInput;
use crate::repository_rule::FrozenStarlarkRepositoryRule;

/// Errors during Starlark repository rule execution.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
enum StarlarkRepoRuleError {
    #[error("Value '{name}' in '{path}' is not a repository_rule")]
    NotARepositoryRule { name: String, path: String },

    #[error("Repository rule implementation returned an error: {0}")]
    ImplementationError(String),
}

/// Convert a `slug_bzlmod` AttrValue to a `repository_ctx` AttrValue.
fn convert_attr_value(value: &slug_bzlmod::RepoAttrValue) -> CtxAttrValue {
    match value {
        slug_bzlmod::RepoAttrValue::String(s) => CtxAttrValue::String(s.clone()),
        slug_bzlmod::RepoAttrValue::Int(i) => CtxAttrValue::Int(*i),
        slug_bzlmod::RepoAttrValue::Bool(b) => CtxAttrValue::Bool(*b),
        slug_bzlmod::RepoAttrValue::None => CtxAttrValue::None,
        slug_bzlmod::RepoAttrValue::StringList(list) => CtxAttrValue::StringList(list.clone()),
        slug_bzlmod::RepoAttrValue::Label(s) => CtxAttrValue::Label(s.clone()),
        slug_bzlmod::RepoAttrValue::Dict(map) => {
            let converted: HashMap<String, CtxAttrValue> = map
                .iter()
                .map(|(k, v)| (k.clone(), convert_attr_value(v)))
                .collect();
            CtxAttrValue::Dict(converted)
        }
    }
}

/// Convert a `CoercedAttr` default value to a `repository_ctx` AttrValue.
/// Mirrors `repository_rule::coerced_attr_to_repo_attr_value` but produces
/// the ctx flavour used directly in `RepositoryContext`.
fn coerced_attr_to_ctx_attr_value(
    attr: &slug_node::attrs::coerced_attr::CoercedAttr,
) -> Option<CtxAttrValue> {
    use slug_node::attrs::coerced_attr::CoercedAttr;
    match attr {
        CoercedAttr::String(s) | CoercedAttr::EnumVariant(s) => {
            let s = s.as_str().to_owned();
            if s.starts_with("//") || s.starts_with('@') || s.starts_with(':') {
                Some(CtxAttrValue::Label(s))
            } else {
                Some(CtxAttrValue::String(s))
            }
        }
        CoercedAttr::Int(i) => Some(CtxAttrValue::Int(*i)),
        CoercedAttr::Bool(b) => Some(CtxAttrValue::Bool(b.0)),
        CoercedAttr::None => Some(CtxAttrValue::None),
        CoercedAttr::Label(label)
        | CoercedAttr::Dep(label)
        | CoercedAttr::SourceLabel(label)
        | CoercedAttr::ConfigurationDep(label)
        | CoercedAttr::SplitTransitionDep(label) => Some(CtxAttrValue::Label(label.to_string())),
        CoercedAttr::OneOf(value, _) => coerced_attr_to_ctx_attr_value(value),
        CoercedAttr::List(list) => {
            let items: Vec<String> = list
                .iter()
                .filter_map(|v| match coerced_attr_to_ctx_attr_value(v)? {
                    CtxAttrValue::String(s) | CtxAttrValue::Label(s) => Some(s),
                    _ => None,
                })
                .collect();
            Some(CtxAttrValue::StringList(items))
        }
        CoercedAttr::Dict(dict) => {
            let entries = dict
                .iter()
                .filter_map(|(k, v)| {
                    let key = match coerced_attr_to_ctx_attr_value(k)? {
                        CtxAttrValue::String(s) | CtxAttrValue::Label(s) => s,
                        _ => return None,
                    };
                    Some((key, coerced_attr_to_ctx_attr_value(v)?))
                })
                .collect();
            Some(CtxAttrValue::Dict(entries))
        }
        _ => None,
    }
}

/// Concrete implementation of Starlark repository rule executor.
pub struct ConcreteStarlarkRepoRuleExecutor;

struct RootLocalBzlPath {
    module_id: String,
    project_path: ProjectRelativePathBuf,
    package: String,
}

struct RootLocalBzlModule {
    content: String,
    package: String,
}

fn repository_rule_label_cell_paths(
    cell_resolver: &CellResolver,
    workspace_root_path: &Path,
    repository_name: &str,
) -> HashMap<String, std::path::PathBuf> {
    let mut cell_paths = HashMap::new();
    for (cell_name, cell_instance) in cell_resolver.cells() {
        let rel_path = cell_instance.path().as_project_relative_path();
        cell_paths.insert(
            cell_name.as_str().to_owned(),
            workspace_root_path.join(rel_path.as_str()),
        );
    }
    for (cell_name, rel_path) in cell_resolver.bzlmod_label_cell_paths() {
        cell_paths
            .entry(cell_name)
            .or_insert_with(|| workspace_root_path.join(rel_path));
    }
    let owner_module =
        slug_bzlmod::parse_canonical_name(repository_name).map(|(owner, _, _)| owner);
    for (cell_name, rel_path) in cell_resolver.bzlmod_label_cell_paths_for_owner(owner_module) {
        cell_paths
            .entry(cell_name)
            .or_insert_with(|| workspace_root_path.join(rel_path));
    }
    cell_paths
}

fn repository_rule_source_repo(
    rule_bzl_path: &str,
    repo_mappings: &slug_bzlmod::RepoMappingSnapshot,
) -> String {
    let Some(rest) = rule_bzl_path
        .strip_prefix("@@")
        .or_else(|| rule_bzl_path.strip_prefix('@'))
    else {
        return String::new();
    };
    let repo = rest.split("//").next().unwrap_or(rest);
    if repo.is_empty() {
        return String::new();
    }
    if repo_mappings.contains_key(repo) {
        return repo.to_owned();
    }
    if let Some(stripped) = repo.strip_suffix('+')
        && repo_mappings.contains_key(stripped)
    {
        return stripped.to_owned();
    }
    repo.to_owned()
}

fn root_local_bzl_path(
    bzl_path: &str,
    current_package: Option<&str>,
) -> slug_error::Result<Option<RootLocalBzlPath>> {
    if bzl_path.starts_with('@') {
        return Ok(None);
    }

    let (package, file, project_path) = if let Some(rest) = bzl_path.strip_prefix("//") {
        if let Some((package, file)) = rest.rsplit_once(':') {
            let project_path = if package.is_empty() {
                file.to_owned()
            } else {
                format!("{package}/{file}")
            };
            (package.to_owned(), file.to_owned(), project_path)
        } else {
            let (package, file) = rest.rsplit_once('/').unwrap_or(("", rest));
            (package.to_owned(), file.to_owned(), rest.to_owned())
        }
    } else if let Some(file) = bzl_path.strip_prefix(':') {
        let package = current_package.unwrap_or("");
        let project_path = if package.is_empty() {
            file.to_owned()
        } else {
            format!("{package}/{file}")
        };
        (package.to_owned(), file.to_owned(), project_path)
    } else {
        return Ok(None);
    };

    let module_id = if package.is_empty() {
        format!("//:{file}")
    } else {
        format!("//{package}:{file}")
    };

    Ok(Some(RootLocalBzlPath {
        module_id,
        project_path: ProjectRelativePath::new(&project_path)?.to_owned(),
        package,
    }))
}

async fn collect_root_local_bzl_modules(
    ctx: &mut DiceComputations<'_>,
    root: RootLocalBzlPath,
) -> slug_error::Result<Option<HashMap<String, RootLocalBzlModule>>> {
    let mut modules = HashMap::new();
    let mut pending = vec![root];

    while let Some(module) = pending.pop() {
        if modules.contains_key(&module.module_id) {
            continue;
        }

        let content = DiceFileComputations::read_project_file(ctx, &module.project_path)
            .await
            .without_package_context_information()
            .with_buck_error_context(|| {
                format!(
                    "Reading root-local repository rule module '{}'",
                    module.module_id
                )
            })?;
        let ast = AstModule::parse(&module.module_id, content.clone(), &Dialect::Standard)
            .map_err(|e| slug_error::slug_error!(slug_error::ErrorTag::Input, "{}", e))?;

        for load in ast.loads() {
            let Some(dep) = root_local_bzl_path(load.module_id, Some(&module.package))? else {
                tracing::debug!(
                    "Skipping local-bit precompute for '{}': load '{}' is not root-local",
                    module.module_id,
                    load.module_id
                );
                return Ok(None);
            };
            if !modules.contains_key(&dep.module_id) {
                pending.push(dep);
            }
        }

        modules.insert(
            module.module_id,
            RootLocalBzlModule {
                content,
                package: module.package,
            },
        );
    }

    Ok(Some(modules))
}

fn eval_root_local_bzl_module(
    module_id: &str,
    modules: &HashMap<String, RootLocalBzlModule>,
    frozen_modules: &mut HashMap<String, FrozenModule>,
    globals: &Globals,
) -> slug_error::Result<FrozenModule> {
    if let Some(module) = frozen_modules.get(module_id) {
        return Ok(module.clone());
    }

    let module_source = modules.get(module_id).ok_or_else(|| {
        slug_error::slug_error!(
            slug_error::ErrorTag::Input,
            "Root-local repository rule module '{}' was not collected",
            module_id
        )
    })?;
    let ast = AstModule::parse(module_id, module_source.content.clone(), &Dialect::Standard)
        .map_err(|e| slug_error::slug_error!(slug_error::ErrorTag::Input, "{}", e))?;

    let mut loaded_modules = Vec::new();
    for load in ast.loads() {
        let dep = root_local_bzl_path(load.module_id, Some(&module_source.package))?.ok_or_else(
            || {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Input,
                    "Load '{}' in '{}' is not root-local",
                    load.module_id,
                    module_id
                )
            },
        )?;
        let frozen = eval_root_local_bzl_module(&dep.module_id, modules, frozen_modules, globals)?;
        loaded_modules.push((load.module_id.to_owned(), frozen));
    }

    let loader_modules: HashMap<&str, &FrozenModule> = loaded_modules
        .iter()
        .map(|(load_id, module)| (load_id.as_str(), module))
        .collect();
    let loader = ReturnFileLoader {
        modules: &loader_modules,
    };
    let module = Module::new();
    {
        let mut eval = Evaluator::new(&module);
        eval.set_loader(&loader);
        eval.eval_module(ast, globals).map_err(|e| {
            slug_error::slug_error!(slug_error::ErrorTag::Input, "{}", e.to_string())
        })?;
    }
    let frozen = module.freeze().map_err(from_freeze_error)?;
    frozen_modules.insert(module_id.to_owned(), frozen.clone());
    Ok(frozen)
}

async fn load_root_local_repository_rule(
    ctx: &mut DiceComputations<'_>,
    rule_bzl_path: &str,
    rule_name: &str,
) -> slug_error::Result<Option<OwnedFrozenValueTyped<FrozenStarlarkRepositoryRule>>> {
    let Some(root) = root_local_bzl_path(rule_bzl_path, None)? else {
        return Ok(None);
    };
    let root_module_id = root.module_id.clone();
    let Some(modules) = collect_root_local_bzl_modules(ctx, root).await? else {
        return Ok(None);
    };

    let mut builder = GlobalsBuilder::standard();
    register_load_natives(&mut builder);
    let globals = builder.build();
    let mut frozen_modules = HashMap::new();
    let loaded_module =
        eval_root_local_bzl_module(&root_module_id, &modules, &mut frozen_modules, &globals)?;

    let rule_value = loaded_module
        .get_any_visibility(rule_name)
        .map_err(|e| from_any_with_tag(e, slug_error::ErrorTag::Input))?
        .0;

    Ok(Some(rule_value.downcast_starlark().map_err(|_| {
        StarlarkRepoRuleError::NotARepositoryRule {
            name: rule_name.to_owned(),
            path: rule_bzl_path.to_owned(),
        }
    })?))
}

async fn load_frozen_repository_rule(
    ctx: &mut DiceComputations<'_>,
    rule_bzl_path: &str,
    rule_name: &str,
) -> slug_error::Result<OwnedFrozenValueTyped<FrozenStarlarkRepositoryRule>> {
    let cell_resolver = ctx.get_cell_resolver().await?;
    let import_path = parse_bzlmod_bzl_path(rule_bzl_path, &cell_resolver)?;

    tracing::debug!("Loading repository rule module from: {}", import_path);

    let loaded_module = ctx
        .get_loaded_module(StarlarkModulePath::LoadFile(&import_path))
        .await
        .buck_error_context(format!(
            "Loading repository rule bzl file: {}",
            rule_bzl_path
        ))?;

    let rule_value = loaded_module
        .env()
        .get_any_visibility(rule_name)
        .map_err(|e| from_any_with_tag(e, slug_error::ErrorTag::Input))?
        .0;

    Ok(rule_value
        .downcast_starlark()
        .map_err(|_| StarlarkRepoRuleError::NotARepositoryRule {
            name: rule_name.to_owned(),
            path: rule_bzl_path.to_owned(),
        })?)
}

#[async_trait]
impl StarlarkRepoRuleExecutorImpl for ConcreteStarlarkRepoRuleExecutor {
    async fn rule_is_local(
        &self,
        ctx: &mut DiceComputations<'_>,
        rule_bzl_path: &str,
        rule_name: &str,
    ) -> slug_error::Result<bool> {
        let Some(frozen_rule) =
            load_root_local_repository_rule(ctx, rule_bzl_path, rule_name).await?
        else {
            return Ok(false);
        };
        Ok(frozen_rule.is_local())
    }

    async fn execute_rule(
        &self,
        ctx: &mut DiceComputations<'_>,
        invocation: &RepositoryInvocation,
        rule_bzl_path: &str,
        rule_name: &str,
        working_dir: &Path,
        repo_env: Arc<BTreeMap<String, String>>,
        repo_mappings: Arc<slug_bzlmod::RepoMappingSnapshot>,
        workspace_id: WorkspaceId,
    ) -> slug_error::Result<StarlarkRepoRuleExecution> {
        tracing::debug!(
            "Executing Starlark repository rule '{}' from '{}' for repo '{}'",
            rule_name,
            rule_bzl_path,
            invocation.name
        );

        let cell_resolver = ctx.get_cell_resolver().await?;
        let frozen_rule = load_frozen_repository_rule(ctx, rule_bzl_path, rule_name).await?;

        tracing::debug!("Found repository rule '{}' in module", frozen_rule.name());

        // 6. Convert attrs from bzlmod AttrValue to repository_ctx AttrValue
        let mut ctx_attrs: HashMap<String, CtxAttrValue> = invocation
            .attrs
            .iter()
            .map(|(k, v)| (k.clone(), convert_attr_value(v)))
            .collect();

        // 6b. Merge in defaults from the rule's declared attrs for any user-
        // unspecified attribute. Matches the extension-context path in
        // repository_rule.rs:478-486.
        for (attr_name, attr_def) in frozen_rule.attrs() {
            if ctx_attrs.contains_key(attr_name) {
                continue;
            }
            if let Some(default) = attr_def.default() {
                if let Some(v) = coerced_attr_to_ctx_attr_value(default) {
                    ctx_attrs.insert(attr_name.clone(), v);
                }
            }
        }

        let repo_attr = RepositoryAttr::new_with_name(invocation.name.clone(), ctx_attrs);

        // 7. Create the RepositoryContext
        let io = ctx.global_data().get_io_provider();
        let workspace_root = io.project_root();
        let workspace_root_path = workspace_root.root().to_path_buf();
        let cell_paths = repository_rule_label_cell_paths(
            &cell_resolver,
            &workspace_root_path,
            &invocation.name,
        );
        let label_source_repo = repository_rule_source_repo(rule_bzl_path, repo_mappings.as_ref());
        let repo_ctx = RepositoryContext::new_with_workspace_root(
            invocation.name.clone(),
            repo_attr,
            working_dir.to_path_buf(),
            workspace_root_path,
        )
        .with_label_resolution(cell_paths)
        .with_label_recording(label_source_repo, repo_mappings)
        .with_repo_env(repo_env);

        tracing::debug!(
            "Invoking repository rule implementation for '{}'",
            invocation.name
        );

        // Plan 39 phase 1.75: expose DICE to `rctx.path(Label)` so it can
        // synchronously materialize cross-repo labels (notably the master
        // git_repository clone that rules_rs's `crate_git_repository`
        // worktree-fans-out from). `with_extension_dice` was originally
        // introduced for module-extension Starlark eval (Plan 36); the same
        // sync->async bridge applies here.
        let invoke_result: Result<(), String> = {
            let impl_fn = frozen_rule.implementation();
            let starlark_module = Module::new();
            let ctx_value = starlark_module.heap().alloc(repo_ctx.clone());
            let label_recorder = repo_ctx.label_recorder();
            let mut eval = Evaluator::new(&starlark_module);
            eval.extra = Some(&label_recorder);
            slug_bzlmod::with_extension_dice(ctx, workspace_id, || {
                eval.eval_function(impl_fn.to_value(), &[ctx_value], &[])
            })
            .map(|_| ())
            .map_err(|e| diagnostic_summary(&e))
        };

        match invoke_result {
            Ok(_) => {
                tracing::info!(
                    "Repository rule '{}' (rule: '{}') completed successfully",
                    invocation.name,
                    rule_name
                );
                let watch_inputs = repo_ctx.watch_inputs()?;
                let recorded_inputs = repo_ctx.recorded_inputs()?;
                track_repository_watch_inputs(ctx, &cell_resolver, workspace_root, &watch_inputs)
                    .await?;
                Ok(StarlarkRepoRuleExecution::new(
                    recorded_inputs,
                    frozen_rule.is_local(),
                ))
            }
            Err(summary) => {
                tracing::debug!(
                    "Repository rule '{}' implementation failed: {}",
                    rule_name,
                    summary
                );
                Err(StarlarkRepoRuleError::ImplementationError(summary).into())
            }
        }
    }
}

async fn track_repository_watch_inputs(
    ctx: &mut DiceComputations<'_>,
    cell_resolver: &CellResolver,
    project_root: &ProjectRoot,
    inputs: &[RepositoryWatchInput],
) -> slug_error::Result<()> {
    for input in inputs {
        match input {
            RepositoryWatchInput::File(path) => {
                let Some(cell_path) = cell_path_for_watch_input(cell_resolver, project_root, path)
                else {
                    continue;
                };
                // Path metadata includes the file digest, so this is enough to
                // invalidate on content edits without requiring watched files
                // to be UTF-8 text.
                let _ = DiceFileComputations::read_path_metadata_if_exists(ctx, cell_path.as_ref())
                    .await?;
            }
            RepositoryWatchInput::Dirents(path) => {
                let Some(cell_path) = cell_path_for_watch_input(cell_resolver, project_root, path)
                else {
                    continue;
                };
                let _ = DiceFileComputations::read_dir(ctx, cell_path.as_ref()).await?;
            }
            RepositoryWatchInput::DirTree(path) => {
                let Some(cell_path) = cell_path_for_watch_input(cell_resolver, project_root, path)
                else {
                    continue;
                };
                track_repository_watch_tree(ctx, cell_path).await?;
            }
        }
    }
    Ok(())
}

async fn track_repository_watch_tree(
    ctx: &mut DiceComputations<'_>,
    root: CellPath,
) -> slug_error::Result<()> {
    let mut pending = vec![root];
    while let Some(path) = pending.pop() {
        let metadata =
            DiceFileComputations::read_path_metadata_if_exists(ctx, path.as_ref()).await?;
        match metadata {
            Some(RawPathMetadata::File(_)) => {}
            Some(RawPathMetadata::Directory) => {
                let entries = DiceFileComputations::read_dir(ctx, path.as_ref()).await?;
                for entry in entries.included.iter() {
                    let child = ForwardRelativePath::new(entry.file_name.as_str())?;
                    pending.push(path.join(child));
                }
            }
            Some(RawPathMetadata::Symlink { .. }) | None => {}
        }
    }
    Ok(())
}

fn cell_path_for_watch_input(
    cell_resolver: &CellResolver,
    project_root: &ProjectRoot,
    path: &Path,
) -> Option<CellPath> {
    let abs = AbsPath::new(path).ok()?;
    cell_resolver
        .get_cell_path_from_abs_path(abs, project_root)
        .ok()
}

fn diagnostic_summary(error: impl std::fmt::Display) -> String {
    const MAX_CHARS: usize = 2000;
    let rendered = format!("{error:#}");
    let mut iter = rendered.char_indices();
    let Some((idx, _)) = iter.nth(MAX_CHARS) else {
        return rendered;
    };
    let omitted = rendered[idx..].chars().count();
    format!(
        "{} ... (truncated; {} chars omitted)",
        &rendered[..idx],
        omitted
    )
}

/// Initialize the late binding for Starlark repository rule execution.
///
/// Called from `init_late_bindings()` in lib.rs.
pub fn init_starlark_repo_rule_executor() {
    slug_bzlmod::STARLARK_REPO_RULE_EXECUTOR_IMPL.init(&ConcreteStarlarkRepoRuleExecutor);
}
