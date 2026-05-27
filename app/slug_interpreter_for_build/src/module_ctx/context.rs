/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! `ModuleContext` — the `ctx.*` object passed to module extension
//! implementation functions. Holds the list of modules using this extension
//! plus helpers to resolve Labels to filesystem paths.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use derive_more::Display;
use starlark::any::ProvidesStaticType;
use starlark::environment::Methods;
use starlark::environment::MethodsStatic;
use starlark::starlark_simple_value;
use starlark::typing::Ty;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

use crate::label_filesystem::LabelFilesystemResolver;
use crate::label_filesystem::RootLabelResolution;
use crate::module_ctx::metadata::FactsValue;
use crate::module_ctx::metadata::empty_facts;
use crate::module_ctx::methods::module_ctx_methods;
use crate::module_ctx::module::BazelModule;
use crate::module_ctx::os::RepositoryOs;
use crate::module_ctx::tags::SerializedTag;
use crate::module_ctx::tags::SerializedTagValue;

// ============================================================================
// ModuleContext - The context object passed to module extension implementations
// ============================================================================

/// Serialized module data for storage in ModuleContext.
#[derive(Debug, Clone, Allocative)]
pub struct SerializedModule {
    /// Module name.
    pub name: String,
    /// Module version.
    pub version: String,
    /// Whether this is the root module.
    pub is_root: bool,
    /// Tags grouped by tag class name.
    pub tags_by_class: HashMap<String, Vec<SerializedTag>>,
}

/// The context object passed to module extension implementation functions.
///
/// ## Working Directory Lifecycle
///
/// Unlike `repository_ctx`, the `module_ctx` working directory is TEMPORARY:
/// - Created at the start of extension execution
/// - Used for any I/O operations during extension evaluation (download, file, execute)
/// - Deleted when the extension completes (regardless of success/failure)
///
/// This is in contrast to `repository_ctx` where the working directory is PERMANENT
/// and becomes the repository output.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<module_ctx>")]
pub struct ModuleContext {
    /// Modules that use this extension.
    modules: Vec<SerializedModule>,
    /// Whether the root module has a non-dev dependency on this extension.
    root_module_has_non_dev_dependency: bool,
    /// TEMPORARY working directory for I/O during extension evaluation.
    /// This is deleted when the extension completes - NOT the repository output.
    /// Use `with_temp_working_dir()` to set this.
    #[allocative(skip)]
    pub(super) working_dir: Option<Arc<PathBuf>>,
    /// Whether the working directory should be deleted when the context is dropped.
    /// Always true for module_ctx (key difference from repository_ctx).
    delete_on_close: bool,
    /// Project root path for resolving Labels to filesystem paths.
    /// Set via `with_label_resolution()`.
    #[allocative(skip)]
    project_root: Option<PathBuf>,
    /// Map of cell_name → absolute filesystem path for Label resolution.
    /// Built from CellResolver before entering Starlark eval.
    #[allocative(skip)]
    cell_paths: HashMap<String, PathBuf>,
    /// Facts returned by a previous execution of this extension.
    #[allocative(skip)]
    facts: serde_json::Value,
    /// Effective command repository environment for this extension execution.
    #[allocative(skip)]
    repo_env: Arc<BTreeMap<String, String>>,
    /// Recorded inputs collected by module_ctx.watch/getenv and declared environ.
    #[allocative(skip)]
    recorded_inputs: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShouldWatch {
    Yes,
    No,
    Auto,
}

impl ShouldWatch {
    pub(crate) fn parse(value: &str) -> starlark::Result<Self> {
        match value {
            "yes" => Ok(Self::Yes),
            "no" => Ok(Self::No),
            "auto" => Ok(Self::Auto),
            _ => Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "bad value for 'watch' parameter; want 'yes', 'no', or 'auto', got {}",
                value
            )
            .into()),
        }
    }
}

starlark_simple_value!(ModuleContext);

impl ModuleContext {
    /// Create a new module context from BazelModule objects (backward compatible).
    pub fn new(modules: Vec<BazelModule>, root_module_has_non_dev_dependency: bool) -> Self {
        let serialized_modules = modules
            .into_iter()
            .map(|m| SerializedModule {
                name: m.name,
                version: m.version,
                is_root: m.is_root,
                tags_by_class: m.tags_by_class,
            })
            .collect();
        Self {
            modules: serialized_modules,
            root_module_has_non_dev_dependency,
            working_dir: None,
            delete_on_close: true,
            project_root: None,
            cell_paths: HashMap::new(),
            facts: empty_facts(),
            repo_env: Arc::new(BTreeMap::new()),
            recorded_inputs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create from serialized module data.
    pub fn from_serialized(
        modules: Vec<SerializedModule>,
        root_module_has_non_dev_dependency: bool,
    ) -> Self {
        Self {
            modules,
            root_module_has_non_dev_dependency,
            working_dir: None,
            delete_on_close: true,
            project_root: None,
            cell_paths: HashMap::new(),
            facts: empty_facts(),
            repo_env: Arc::new(BTreeMap::new()),
            recorded_inputs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create an empty module context (for testing).
    pub fn empty() -> Self {
        Self {
            modules: Vec::new(),
            root_module_has_non_dev_dependency: false,
            working_dir: None,
            delete_on_close: true,
            project_root: None,
            cell_paths: HashMap::new(),
            facts: empty_facts(),
            repo_env: Arc::new(BTreeMap::new()),
            recorded_inputs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Set the temporary working directory for this module context.
    ///
    /// This directory is used for any I/O operations (download, file, execute)
    /// during extension evaluation. Unlike repository_ctx, this directory is
    /// TEMPORARY and will be deleted after the extension completes.
    ///
    /// # Arguments
    ///
    /// * `dir` - The path to the temporary working directory
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ctx = ModuleContext::empty()
    ///     .with_temp_working_dir(temp_dir);
    /// ```
    pub fn with_temp_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(Arc::new(dir));
        self.delete_on_close = true; // Ensure this is always true
        self
    }

    /// Set the project root and cell path map for Label-to-path resolution.
    ///
    /// This enables `module_ctx.path(Label)` and `module_ctx.execute([Label, ...])`
    /// to resolve Label arguments to filesystem paths. The cell_paths map is built
    /// from the CellResolver before entering Starlark evaluation.
    pub fn with_label_resolution(
        mut self,
        project_root: PathBuf,
        cell_paths: HashMap<String, PathBuf>,
    ) -> Self {
        self.project_root = Some(project_root);
        self.cell_paths = cell_paths;
        self
    }

    /// Set facts from a previous execution of this extension.
    pub fn with_facts(mut self, facts: serde_json::Value) -> Self {
        self.facts = match facts {
            serde_json::Value::Object(_) => facts,
            _ => empty_facts(),
        };
        self
    }

    /// Set the effective repository environment for module extension APIs.
    pub fn with_repo_env(mut self, repo_env: Arc<BTreeMap<String, String>>) -> Self {
        self.repo_env = repo_env;
        self
    }

    /// Apply tag class defaults from a frozen extension's tag classes.
    ///
    /// In Bazel, tag attributes have declared defaults (e.g., `attr.string_list_dict(default={})`).
    /// When a tag is used in MODULE.bazel without specifying all attributes, the missing
    /// attributes get their default values. Slug's tag serialization only stores explicitly
    /// provided values. This method fills in missing values from the tag class schema.
    ///
    /// `defaults` maps tag_class_name → (attr_name → SerializedTagValue).
    pub fn apply_tag_class_defaults(
        &mut self,
        defaults: &HashMap<String, Vec<(String, SerializedTagValue)>>,
    ) {
        for module in &mut self.modules {
            for (class_name, tags) in &mut module.tags_by_class {
                if let Some(class_defaults) = defaults.get(class_name) {
                    for tag in tags.iter_mut() {
                        let existing_keys: std::collections::HashSet<String> =
                            tag.kwargs.iter().map(|(k, _)| k.clone()).collect();
                        for (attr_name, default_val) in class_defaults {
                            if !existing_keys.contains(attr_name) {
                                tag.kwargs.push((attr_name.clone(), default_val.clone()));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Get the working directory, if set.
    pub fn working_dir(&self) -> Option<&Path> {
        self.working_dir.as_ref().map(|p| p.as_path())
    }

    /// Check if this context has a working directory set.
    pub fn has_working_dir(&self) -> bool {
        self.working_dir.is_some()
    }

    /// Check if the working directory should be deleted on close.
    /// Always returns true for module_ctx.
    pub fn should_delete_working_dir(&self) -> bool {
        self.delete_on_close
    }

    /// Resolve a path relative to the working directory.
    /// Returns None if no working directory is set.
    pub fn resolve_path(&self, path: &str) -> Option<PathBuf> {
        self.working_dir.as_ref().map(|base| {
            if Path::new(path).is_absolute() {
                PathBuf::from(path)
            } else {
                base.join(path)
            }
        })
    }

    /// Resolve a Label string to an absolute filesystem path.
    ///
    /// This is the slug equivalent of Bazel's `getPathFromLabel()` from
    /// `StarlarkBaseExternalContext`. In Bazel, this triggers Skyframe-based
    /// repo materialization. In slug, we use a pre-built cell path map.
    ///
    /// Label format: `@@repo//pkg:target` or `@repo//pkg:target`
    /// Returns None if the label can't be resolved.
    pub fn resolve_label_to_filesystem_path(&self, label_str: &str) -> Option<PathBuf> {
        let project_root = self.project_root.as_ref()?;
        LabelFilesystemResolver::new(project_root)
            .with_project_root(Some(project_root))
            .with_cell_paths(&self.cell_paths)
            .with_root_label_resolution(RootLabelResolution::ProjectAbsolute)
            .resolve_label_string(label_str)
    }

    /// Get the modules.
    pub fn get_modules(&self) -> &[SerializedModule] {
        &self.modules
    }

    /// Effective repository environment for module_ctx.getenv/os.environ.
    pub fn repo_env(&self) -> &BTreeMap<String, String> {
        &self.repo_env
    }

    /// Return recorded inputs collected during module extension execution.
    pub fn recorded_inputs(&self) -> slug_error::Result<Vec<String>> {
        self.recorded_inputs
            .lock()
            .map(|inputs| inputs.clone())
            .map_err(|_| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "module_ctx recorded input lock poisoned"
                )
            })
    }

    pub fn record_file_input(&self, path: &Path) -> starlark::Result<()> {
        self.maybe_record_file_input(path, ShouldWatch::Yes)
    }

    pub(crate) fn maybe_record_file_input(
        &self,
        path: &Path,
        should_watch: ShouldWatch,
    ) -> starlark::Result<()> {
        if should_watch == ShouldWatch::No {
            return Ok(());
        }
        if let Some(working_dir) = self.working_dir()
            && path.starts_with(working_dir)
        {
            if should_watch == ShouldWatch::Auto {
                return Ok(());
            }
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "attempted to watch path under working directory"
            )
            .into());
        }
        let Some(recorded_path) = self.recorded_path_for_watch(path, should_watch)? else {
            return Ok(());
        };
        let recorded = slug_bzlmod::recorded_file_input_with_recorded_path(&recorded_path, path)
            .map_err(|e| {
                starlark::Error::from(slug_error::slug_error!(
                    slug_error::ErrorTag::Input,
                    "Failed to record module_ctx.watch input '{}': {}",
                    path.display(),
                    e
                ))
            })?;
        self.record_input(recorded)
    }

    fn recorded_path_for_watch(
        &self,
        path: &Path,
        should_watch: ShouldWatch,
    ) -> starlark::Result<Option<PathBuf>> {
        let mut cell_paths: Vec<_> =
            self.cell_paths
                .iter()
                .filter(|(cell_name, cell_path)| {
                    !cell_name.is_empty()
                        && !slug_core::cells::is_root_cell_name(cell_name)
                        && !self.project_root.as_ref().is_some_and(|project_root| {
                            project_root.as_path() == cell_path.as_path()
                        })
                })
                .collect();
        cell_paths.sort_by(|(_, left), (_, right)| {
            right
                .components()
                .count()
                .cmp(&left.components().count())
                .then_with(|| right.cmp(left))
        });
        for (cell_name, cell_path) in cell_paths {
            if let Ok(repo_relative) = path.strip_prefix(cell_path) {
                let recorded = format!("@@{}+//{}", cell_name, repo_relative.to_string_lossy());
                return Ok(Some(PathBuf::from(recorded)));
            }
        }
        if let Some(project_root) = &self.project_root
            && let Ok(workspace_relative) = path.strip_prefix(project_root)
        {
            let recorded = format!("@@//{}", workspace_relative.to_string_lossy());
            return Ok(Some(PathBuf::from(recorded)));
        }
        if should_watch == ShouldWatch::Auto {
            return Ok(None);
        }
        Err(slug_error::slug_error!(
            slug_error::ErrorTag::Input,
            "attempted to watch path outside workspace, but it's prohibited in the current context"
        )
        .into())
    }

    pub fn record_env_input(&self, name: &str) -> starlark::Result<()> {
        let recorded =
            slug_bzlmod::recorded_env_input(name, self.repo_env.get(name).map(String::as_str));
        self.record_input(recorded)
    }

    fn record_input(&self, recorded: String) -> starlark::Result<()> {
        let mut inputs = self.recorded_inputs.lock().map_err(|_| {
            starlark::Error::from(slug_error::slug_error!(
                slug_error::ErrorTag::Tier0,
                "module_ctx recorded input lock poisoned"
            ))
        })?;
        let (identity, value) = split_recorded_input(&recorded).ok_or_else(|| {
            starlark::Error::from(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "malformed recorded input '{}'",
                recorded
            ))
        })?;
        if let Some(existing) = inputs
            .iter()
            .find(|input| split_recorded_input(input).is_some_and(|(key, _)| key == identity))
        {
            let (_, existing_value) = split_recorded_input(existing).expect("checked above");
            if existing_value == value {
                return Ok(());
            }
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Input,
                "Conflicting values recorded for input {}: '{}' vs '{}'",
                identity,
                existing_value,
                value
            )
            .into());
        }
        inputs.push(recorded);
        Ok(())
    }
}

fn split_recorded_input(recorded: &str) -> Option<(&str, &str)> {
    let (identity, value) = recorded.split_once(' ')?;
    if identity.is_empty() {
        return None;
    }
    Some((identity, value))
}

#[starlark_value(type = "module_ctx")]
impl<'v> StarlarkValue<'v> for ModuleContext {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "modules"
                | "os"
                | "root_module_has_non_dev_dependency"
                | "is_isolated"
                | "root_module_direct_deps"
                | "root_module_direct_dev_deps"
                | "facts"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "modules" => {
                let modules: Vec<Value<'v>> = self
                    .modules
                    .iter()
                    .map(|m| {
                        heap.alloc(BazelModule::with_tags(
                            m.name.clone(),
                            m.version.clone(),
                            m.is_root,
                            m.tags_by_class.clone(),
                        ))
                    })
                    .collect();
                Some(heap.alloc(modules))
            }
            "os" => Some(heap.alloc(RepositoryOs::new_with_environ(self.repo_env.clone()))),
            "root_module_has_non_dev_dependency" => {
                Some(Value::new_bool(self.root_module_has_non_dev_dependency))
            }
            // Whether this extension is isolated (Bazel 7.1+)
            "is_isolated" => Some(Value::new_bool(false)),
            // Root module's direct (non-dev) bazel_dep labels
            "root_module_direct_deps" => Some(Value::new_none()),
            // Root module's direct dev bazel_dep labels
            "root_module_direct_dev_deps" => Some(Value::new_none()),
            "facts" => Some(heap.alloc(FactsValue::new(self.facts.clone()))),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "modules".to_owned(),
            "os".to_owned(),
            "root_module_has_non_dev_dependency".to_owned(),
            "is_isolated".to_owned(),
            "root_module_direct_deps".to_owned(),
            "root_module_direct_dev_deps".to_owned(),
            "facts".to_owned(),
        ]
    }

    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(module_ctx_methods)
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}
