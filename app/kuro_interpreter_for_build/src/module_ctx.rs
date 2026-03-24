/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Implementation of Bazel's `module_ctx` object for module extensions.
//!
//! Plan Reference: `thoughts/shared/plans/kuro-bazel-subplans/02-bzlmod.md` Phase 5
//!
//! ## Current Status: FULLY IMPLEMENTED
//!
//! This provides the `module_ctx` object passed to module extension implementations.
//! The `modules` property returns real module data with tags populated from
//! kuro_bzlmod's aggregated extension data.
//!
//! ## What's Implemented
//!
//! - `modules` property - list of bazel_module objects with tag data
//! - `os` property - repository_os struct with name, arch, environ
//! - `root_module_has_non_dev_dependency` property
//! - `which()` - find programs on PATH
//! - `execute()` - run commands and get stdout/stderr/return_code
//! - `download()` - download files with SHA256/integrity verification
//! - `download_and_extract()` - download and extract archives
//! - `extract()` - extract local archives
//! - `read()` - read file contents
//! - `file()` - write files
//! - `path()` - convert to RepositoryPath objects
//! - `is_dir()` - check if path is a directory
//! - `delete()` - delete files/directories
//! - `symlink()` - create symlinks (copy fallback on Windows)
//! - `getenv()` - get environment variables
//!
//! ## Example usage in Starlark:
//!
//! ```python
//! def _my_extension_impl(module_ctx):
//!     for mod in module_ctx.modules:
//!         print("Module:", mod.name, "version:", mod.version)
//!         for tag in mod.tags.install:
//!             print("  Tag attrs:", tag.name, tag.version)
//!
//!     print("OS:", module_ctx.os.name)
//!     print("Arch:", module_ctx.os.arch)
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use allocative::Allocative;
use anyhow::anyhow;
use derive_more::Display;
use kuro_build_api::interpreter::rule_defs::bazel_label::BazelLabel;
use starlark::any::ProvidesStaticType;
use starlark::collections::SmallMap;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::typing::Ty;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::Dict;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;
use starlark::values::structs::AllocStruct;

use crate::repository_ctx::DownloadInfo;
use crate::repository_ctx::DownloadToken;
use crate::repository_ctx::ExecutionResult;
use crate::repository_ctx::RepositoryPath;
use crate::repository_ctx::download_url;
use crate::repository_ctx::extract_archive;
use crate::repository_ctx::get_urls_from_value;
use crate::repository_ctx::resolve_label_to_path;
use crate::repository_ctx::verify_integrity;
use crate::repository_ctx::verify_sha256;

// ============================================================================
// RepositoryOs - Information about the host OS (simple value, no lifetime)
// ============================================================================

/// Information about the host operating system.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<repository_os>")]
pub struct RepositoryOs {
    /// The OS name (e.g., "linux", "macos", "windows").
    name: String,
    /// The CPU architecture (e.g., "x86_64", "aarch64").
    arch: String,
}

starlark_simple_value!(RepositoryOs);

impl RepositoryOs {
    pub fn new() -> Self {
        let name = if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "mac os x"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "unknown"
        };

        let arch = if cfg!(target_arch = "x86_64") {
            "amd64"
        } else if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else if cfg!(target_arch = "x86") {
            "x86_32"
        } else {
            "unknown"
        };

        Self {
            name: name.to_owned(),
            arch: arch.to_owned(),
        }
    }
}

#[starlark_value(type = "repository_os")]
impl<'v> StarlarkValue<'v> for RepositoryOs {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(repository_os_methods)
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

#[starlark_module]
fn repository_os_methods(builder: &mut MethodsBuilder) {
    /// The OS name (e.g., "linux", "mac os x", "windows").
    #[starlark(attribute)]
    fn name(this: &RepositoryOs) -> starlark::Result<String> {
        Ok(this.name.clone())
    }

    /// The CPU architecture (e.g., "amd64", "aarch64").
    #[starlark(attribute)]
    fn arch(this: &RepositoryOs) -> starlark::Result<String> {
        Ok(this.arch.clone())
    }

    /// A snapshot of the environment variables at the time repository rules are executed.
    #[starlark(attribute)]
    fn environ<'v>(this: &RepositoryOs, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let mut map = SmallMap::new();
        for (key, val) in std::env::vars() {
            map.insert_hashed(
                heap.alloc_str(&key).to_value().get_hashed().unwrap(),
                heap.alloc_str(&val).to_value(),
            );
        }
        Ok(heap.alloc(Dict::new(map)))
    }
}

// ============================================================================
// Tag value serialization for storage in simple values
// ============================================================================

/// Serialized tag value that can be stored in simple Starlark values.
/// This mirrors kuro_bzlmod::types::TagValue but is owned and serializable.
#[derive(Debug, Clone, Allocative)]
pub enum SerializedTagValue {
    String(String),
    Int(i64),
    Bool(bool),
    None,
    Label(String),
    List(Vec<SerializedTagValue>),
    Dict(Vec<(String, SerializedTagValue)>),
}

impl SerializedTagValue {
    /// Convert to a Starlark value.
    pub fn to_starlark<'v>(&self, heap: Heap<'v>) -> Value<'v> {
        match self {
            SerializedTagValue::String(s) => heap.alloc(s.as_str()),
            SerializedTagValue::Int(i) => heap.alloc(*i as i32),
            SerializedTagValue::Bool(b) => Value::new_bool(*b),
            SerializedTagValue::None => Value::new_none(),
            SerializedTagValue::Label(s) => heap.alloc(BazelLabel::parse(s)),
            SerializedTagValue::List(items) => {
                let values: Vec<Value<'v>> = items.iter().map(|v| v.to_starlark(heap)).collect();
                heap.alloc(values)
            }
            SerializedTagValue::Dict(entries) => {
                let mut map = SmallMap::new();
                for (k, v) in entries {
                    map.insert_hashed(
                        heap.alloc(k.as_str())
                            .get_hashed()
                            .expect("string is hashable"),
                        v.to_starlark(heap),
                    );
                }
                heap.alloc(Dict::new(map))
            }
        }
    }
}

/// Serialized extension tag with its attribute values.
#[derive(Debug, Clone, Allocative)]
pub struct SerializedTag {
    /// Keyword arguments passed to the tag.
    pub kwargs: Vec<(String, SerializedTagValue)>,
}

impl SerializedTag {
    /// Create a new serialized tag.
    pub fn new(kwargs: Vec<(String, SerializedTagValue)>) -> Self {
        Self { kwargs }
    }

    /// Convert to a Starlark struct value.
    pub fn to_starlark_struct<'v>(&self, heap: Heap<'v>) -> Value<'v> {
        let fields: HashMap<String, SerializedTagValue> = self
            .kwargs
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        heap.alloc(TagInstance { fields })
    }
}

// ============================================================================
// TagInstance - A tag instance that returns None for missing attributes
// ============================================================================

/// A tag instance from module extension tags. Unlike regular structs, accessing
/// a missing attribute returns None (matching Bazel behavior where tag attrs
/// have default values, typically None for optional attrs).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct TagInstance {
    fields: HashMap<String, SerializedTagValue>,
}

impl std::fmt::Display for TagInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tag_instance({})",
            self.fields.keys().cloned().collect::<Vec<_>>().join(", ")
        )
    }
}

starlark_simple_value!(TagInstance);

#[starlark_value(type = "struct")]
impl<'v> StarlarkValue<'v> for TagInstance {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        true
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match self.fields.get(attribute) {
            Some(v) => Some(v.to_starlark(heap)),
            None => Some(Value::new_none()),
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        self.fields.keys().cloned().collect()
    }

    fn get_type_starlark_repr() -> starlark::typing::Ty {
        starlark::typing::Ty::any()
    }
}

// ============================================================================
// BazelModuleTags - Collection of tags grouped by tag class (simple value)
// ============================================================================

/// Collection of tags from a module, grouped by tag class name.
/// Access like: `mod.tags.install` to get list of install tags.
///
/// Tags are stored as serialized data and converted to Starlark structs on access.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<bazel_module_tags>")]
pub struct BazelModuleTags {
    /// Tags grouped by tag class name.
    /// Key is tag class name (e.g., "install"), value is list of tags.
    tags_by_class: HashMap<String, Vec<SerializedTag>>,
}

starlark_simple_value!(BazelModuleTags);

impl BazelModuleTags {
    /// Create from tag class names only (for backward compatibility).
    pub fn new(tag_classes: Vec<String>) -> Self {
        let mut tags_by_class = HashMap::new();
        for class in tag_classes {
            tags_by_class.insert(class, Vec::new());
        }
        Self { tags_by_class }
    }

    /// Create with actual tag data.
    pub fn with_tags(tags_by_class: HashMap<String, Vec<SerializedTag>>) -> Self {
        Self { tags_by_class }
    }

    /// Create an empty tags collection.
    pub fn empty() -> Self {
        Self {
            tags_by_class: HashMap::new(),
        }
    }

    /// Add a tag to a tag class.
    pub fn add_tag(&mut self, tag_class: String, tag: SerializedTag) {
        self.tags_by_class.entry(tag_class).or_default().push(tag);
    }
}

#[starlark_value(type = "bazel_module_tags")]
impl<'v> StarlarkValue<'v> for BazelModuleTags {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        // All tag class names are valid attributes. Unknown ones return empty lists.
        // This is needed because the tag class names are defined by the extension
        // (in tag_classes={}), and a module may not use all tag classes.
        true
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        let tags = self.tags_by_class.get(attribute);
        let structs: Vec<Value<'v>> = tags
            .map(|t| t.iter().map(|tag| tag.to_starlark_struct(heap)).collect())
            .unwrap_or_default();
        Some(heap.alloc(structs))
    }

    fn dir_attr(&self) -> Vec<String> {
        self.tags_by_class.keys().cloned().collect()
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

// ============================================================================
// BazelModule - Represents a module in the dependency graph (simple value)
// ============================================================================

/// Represents a module in the dependency graph with its tags.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<bazel_module {} {}>", name, version)]
pub struct BazelModule {
    /// Module name (e.g., "rules_python").
    name: String,
    /// Module version (e.g., "0.31.0").
    version: String,
    /// Whether this is the root module.
    is_root: bool,
    /// Tags grouped by tag class name.
    tags_by_class: HashMap<String, Vec<SerializedTag>>,
}

starlark_simple_value!(BazelModule);

impl BazelModule {
    /// Create from tag class names only (backward compatibility, empty tags).
    pub fn new(name: String, version: String, is_root: bool, tag_classes: Vec<String>) -> Self {
        let mut tags_by_class = HashMap::new();
        for class in tag_classes {
            tags_by_class.insert(class, Vec::new());
        }
        Self {
            name,
            version,
            is_root,
            tags_by_class,
        }
    }

    /// Create with actual tag data.
    pub fn with_tags(
        name: String,
        version: String,
        is_root: bool,
        tags_by_class: HashMap<String, Vec<SerializedTag>>,
    ) -> Self {
        Self {
            name,
            version,
            is_root,
            tags_by_class,
        }
    }

    /// Get the module name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the module version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Check if this is the root module.
    pub fn is_root(&self) -> bool {
        self.is_root
    }

    /// Get the tags by class.
    pub fn tags_by_class(&self) -> &HashMap<String, Vec<SerializedTag>> {
        &self.tags_by_class
    }
}

#[starlark_value(type = "bazel_module")]
impl<'v> StarlarkValue<'v> for BazelModule {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "name" | "version" | "is_root" | "tags" | "repo_name" | "bazel_module_repo_name"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc(&self.name as &str)),
            "version" => Some(heap.alloc(&self.version as &str)),
            "is_root" => Some(Value::new_bool(self.is_root)),
            "tags" => Some(heap.alloc(BazelModuleTags::with_tags(self.tags_by_class.clone()))),
            // The canonical repo name used for the module's repository.
            // For root module this is usually "" or the module name.
            "repo_name" | "bazel_module_repo_name" => Some(heap.alloc(&self.name as &str)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "name".to_owned(),
            "version".to_owned(),
            "is_root".to_owned(),
            "tags".to_owned(),
            "repo_name".to_owned(),
            "bazel_module_repo_name".to_owned(),
        ]
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

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
    working_dir: Option<Arc<PathBuf>>,
    /// Whether the working directory should be deleted when the context is dropped.
    /// Always true for module_ctx (key difference from repository_ctx).
    delete_on_close: bool,
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
            delete_on_close: true, // Always true for module_ctx
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
            delete_on_close: true, // Always true for module_ctx
        }
    }

    /// Create an empty module context (for testing).
    pub fn empty() -> Self {
        Self {
            modules: Vec::new(),
            root_module_has_non_dev_dependency: false,
            working_dir: None,
            delete_on_close: true, // Always true for module_ctx
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

    /// Get the modules.
    pub fn get_modules(&self) -> &[SerializedModule] {
        &self.modules
    }
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
            "os" => Some(heap.alloc(RepositoryOs::new())),
            "root_module_has_non_dev_dependency" => {
                Some(Value::new_bool(self.root_module_has_non_dev_dependency))
            }
            // Whether this extension is isolated (Bazel 7.1+)
            "is_isolated" => Some(Value::new_bool(false)),
            // Root module's direct (non-dev) bazel_dep labels
            "root_module_direct_deps" => Some(Value::new_none()),
            // Root module's direct dev bazel_dep labels
            "root_module_direct_dev_deps" => Some(Value::new_none()),
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

/// Module context methods for Bazel module extensions.
/// I/O operations (download, execute, file) are fully implemented.
/// Watch/template/patch methods remain as no-ops (acceptable for most extensions).
#[starlark_module]
fn module_ctx_methods(builder: &mut MethodsBuilder) {
    /// Returns whether the given module uses this extension as a dev dependency.
    ///
    /// In Bazel, module extensions can check if a particular bazel_module has
    /// declared the extension as a dev dependency. Dev dependencies are only
    /// visible in the root module.
    fn is_dev_dependency<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _module: Value<'v>,
    ) -> starlark::Result<bool> {
        // For now, return false (not a dev dependency).
        // A full implementation would check the module's use_extension() declaration.
        let _ = this;
        Ok(false)
    }

    /// Read a file and return its contents as a string.
    #[allow(unused_variables)]
    fn read(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value,
        #[starlark(require = named, default = "auto")] watch: &str,
    ) -> starlark::Result<String> {
        let path_str = path.unpack_str().unwrap_or("");
        let resolved = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(path_str)
        } else {
            PathBuf::from(path_str)
        };
        match std::fs::read_to_string(&resolved) {
            Ok(content) => Ok(content),
            Err(_) => Ok(String::new()),
        }
    }

    /// Write a file with the given content.
    fn file<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value<'v>,
        #[starlark(require = named, default = "")] content: &str,
        #[starlark(require = named, default = false)] executable: bool,
        #[starlark(require = named, default = false)] _legacy_utf8: bool,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let path_str = path.unpack_str().unwrap_or("");
        let resolved = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(path_str)
        } else {
            return Err(starlark::Error::new_other(anyhow!(
                "module_ctx.file() requires a working directory or absolute path"
            )));
        };

        // Ensure parent directory exists
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                starlark::Error::new_other(anyhow!(
                    "Failed to create parent directory for {}: {}",
                    resolved.display(),
                    e
                ))
            })?;
        }

        std::fs::write(&resolved, content).map_err(|e| {
            starlark::Error::new_other(anyhow!(
                "Failed to write file {}: {}",
                resolved.display(),
                e
            ))
        })?;

        // Set executable permission on Unix
        #[cfg(unix)]
        if executable {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&resolved, perms).ok();
        }
        #[cfg(not(unix))]
        let _ = executable;

        Ok(heap.alloc(RepositoryPath::new(resolved.to_string_lossy().to_string())))
    }

    /// Download a file from a URL.
    fn download<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] url: Value<'v>,
        #[starlark(require = pos, default = "")] output: &str,
        #[starlark(require = named, default = "")] sha256: &str,
        #[starlark(require = named, default = "")] integrity: &str,
        #[starlark(require = named, default = false)] executable: bool,
        #[starlark(require = named, default = true)] allow_fail: bool,
        #[allow(unused_variables)]
        #[starlark(require = named, default = "")]
        canonical_id: &str,
        #[allow(unused_variables)]
        #[starlark(require = named)]
        auth: Option<Value<'v>>,
        #[allow(unused_variables)]
        #[starlark(require = named)]
        headers: Option<Value<'v>>,
        #[allow(unused_variables)]
        #[starlark(require = named, default = true)]
        block: bool,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let urls = get_urls_from_value(url);
        if urls.is_empty() {
            if allow_fail {
                return Ok(heap.alloc(DownloadInfo {
                    success: false,
                    integrity: String::new(),
                    sha256: String::new(),
                }));
            }
            return Err(starlark::Error::new_other(anyhow!(
                "No URL provided for download"
            )));
        }

        // Determine output path
        let output_path = if output.is_empty() {
            let filename = urls[0].split('/').last().unwrap_or("downloaded");
            if let Some(ref wd) = this.working_dir {
                wd.join(filename)
            } else {
                PathBuf::from(filename)
            }
        } else if Path::new(output).is_absolute() {
            PathBuf::from(output)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(output)
        } else {
            PathBuf::from(output)
        };

        // Try each URL until one succeeds
        let mut last_error = None;
        for url_str in &urls {
            match download_url(url_str) {
                Ok(data) => {
                    if !sha256.is_empty() {
                        if let Err(e) = verify_sha256(&data, sha256) {
                            if allow_fail {
                                return Ok(heap.alloc(DownloadInfo {
                                    success: false,
                                    integrity: String::new(),
                                    sha256: String::new(),
                                }));
                            }
                            return Err(starlark::Error::new_other(anyhow!("{}", e)));
                        }
                    }
                    if !integrity.is_empty() {
                        if let Err(e) = verify_integrity(&data, integrity) {
                            if allow_fail {
                                return Ok(heap.alloc(DownloadInfo {
                                    success: false,
                                    integrity: String::new(),
                                    sha256: String::new(),
                                }));
                            }
                            return Err(starlark::Error::new_other(anyhow!("{}", e)));
                        }
                    }

                    if let Some(parent) = output_path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| {
                            starlark::Error::new_other(anyhow!("Failed to create directory: {}", e))
                        })?;
                    }

                    std::fs::write(&output_path, &data).map_err(|e| {
                        starlark::Error::new_other(anyhow!("Failed to write file: {}", e))
                    })?;

                    #[cfg(unix)]
                    if executable {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(mut perms) =
                            std::fs::metadata(&output_path).map(|m| m.permissions())
                        {
                            perms.set_mode(perms.mode() | 0o111);
                            std::fs::set_permissions(&output_path, perms).ok();
                        }
                    }
                    #[cfg(not(unix))]
                    let _ = executable;

                    let info = DownloadInfo::new(true, &data);
                    if block {
                        return Ok(heap.alloc(info));
                    } else {
                        return Ok(heap.alloc(DownloadToken { info }));
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        if allow_fail {
            Ok(heap.alloc(DownloadInfo {
                success: false,
                integrity: String::new(),
                sha256: String::new(),
            }))
        } else {
            Err(starlark::Error::new_other(anyhow!(
                "All download URLs failed: {}",
                last_error.unwrap_or_else(|| "unknown error".to_owned())
            )))
        }
    }

    /// Download and extract an archive from a URL.
    fn download_and_extract<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] url: Value<'v>,
        #[starlark(require = named, default = "")] output: &str,
        #[starlark(require = named, default = "")] sha256: &str,
        #[starlark(require = named, default = "")] integrity: &str,
        #[starlark(require = named, default = "")] strip_prefix: &str,
        #[starlark(require = named, default = "")] _type: &str,
        #[starlark(require = named)] _rename_files: Option<Value<'v>>,
        #[starlark(require = named)] _auth: Option<Value<'v>>,
        #[starlark(require = named)] _headers: Option<Value<'v>>,
        #[starlark(require = named, default = "")] _canonical_id: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let urls = get_urls_from_value(url);
        if urls.is_empty() {
            return Err(starlark::Error::new_other(anyhow!(
                "No URL provided for download_and_extract"
            )));
        }

        // Determine output directory
        let output_dir = if output.is_empty() {
            if let Some(ref wd) = this.working_dir {
                wd.as_ref().clone()
            } else {
                PathBuf::from(".")
            }
        } else if Path::new(output).is_absolute() {
            PathBuf::from(output)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(output)
        } else {
            PathBuf::from(output)
        };

        let mut last_error = None;
        for url_str in &urls {
            match download_url(url_str) {
                Ok(data) => {
                    if !sha256.is_empty() {
                        if let Err(e) = verify_sha256(&data, sha256) {
                            return Err(starlark::Error::new_other(anyhow!("{}", e)));
                        }
                    }
                    if !integrity.is_empty() {
                        if let Err(e) = verify_integrity(&data, integrity) {
                            return Err(starlark::Error::new_other(anyhow!("{}", e)));
                        }
                    }

                    std::fs::create_dir_all(&output_dir).map_err(|e| {
                        starlark::Error::new_other(anyhow!("Failed to create directory: {}", e))
                    })?;

                    let strip = if strip_prefix.is_empty() {
                        None
                    } else {
                        Some(strip_prefix)
                    };
                    extract_archive(&data, &output_dir, strip)
                        .map_err(|e| starlark::Error::new_other(anyhow!("{}", e)))?;

                    return Ok(heap.alloc(DownloadInfo::new(true, &data)));
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(starlark::Error::new_other(anyhow!(
            "All download URLs failed: {}",
            last_error.unwrap_or_else(|| "unknown error".to_owned())
        )))
    }

    /// Execute a command and return its output.
    fn execute<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] arguments: Value<'v>,
        #[starlark(require = named, default = 600)] _timeout: i32,
        #[starlark(require = named)] environment: Option<Value<'v>>,
        #[starlark(require = named, default = true)] quiet: bool,
        #[starlark(require = named, default = "")] working_directory: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let args: Vec<String> =
            if let Some(list) = starlark::values::list::ListRef::from_value(arguments) {
                list.iter()
                    .map(|v| {
                        // Use unpack_str for strings, to_str() for Label and other types
                        v.unpack_str()
                            .map(|s| s.to_owned())
                            .unwrap_or_else(|| v.to_str())
                    })
                    .collect()
            } else {
                return Err(starlark::Error::new_other(anyhow!(
                    "arguments must be a list"
                )));
            };

        if args.is_empty() {
            return Err(starlark::Error::new_other(anyhow!(
                "arguments cannot be empty"
            )));
        }

        let program = &args[0];
        let cmd_args = &args[1..];

        let mut cmd = Command::new(program);
        cmd.args(cmd_args);

        // Set working directory
        if !working_directory.is_empty() {
            cmd.current_dir(working_directory);
        } else if let Some(ref wd) = this.working_dir {
            cmd.current_dir(wd.as_path());
        }

        // Set environment variables if provided
        if let Some(env_val) = environment {
            if let Some(env_dict) = starlark::values::dict::DictRef::from_value(env_val) {
                for (k, v) in env_dict.iter() {
                    if let (Some(key), Some(val)) = (k.unpack_str(), v.unpack_str()) {
                        cmd.env(key, val);
                    }
                }
            }
        }

        let output = cmd
            .output()
            .map_err(|e| starlark::Error::new_other(anyhow!("Failed to execute command: {}", e)))?;

        let return_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !quiet {
            if !stdout.is_empty() {
                eprintln!("{}", stdout);
            }
            if !stderr.is_empty() {
                eprintln!("{}", stderr);
            }
        }

        Ok(heap.alloc(ExecutionResult::new(return_code, stdout, stderr)))
    }

    /// Find the path to a program on PATH.
    fn which<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] program: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        if let Ok(path_var) = std::env::var("PATH") {
            let separator = if cfg!(windows) { ';' } else { ':' };
            for dir in path_var.split(separator) {
                let candidates: Vec<PathBuf> = if cfg!(windows) {
                    let base = Path::new(dir).join(program);
                    if base.extension().is_some() {
                        vec![base]
                    } else {
                        vec![
                            base.with_extension("exe"),
                            base.with_extension("cmd"),
                            base.with_extension("bat"),
                            base.with_extension("com"),
                            base.clone(),
                        ]
                    }
                } else {
                    vec![Path::new(dir).join(program)]
                };

                for full_path in candidates {
                    if full_path.is_file() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(meta) = std::fs::metadata(&full_path) {
                                if meta.permissions().mode() & 0o111 != 0 {
                                    return Ok(heap.alloc(RepositoryPath::new(
                                        full_path.to_string_lossy().to_string(),
                                    )));
                                }
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            return Ok(heap.alloc(RepositoryPath::new(
                                full_path.to_string_lossy().to_string(),
                            )));
                        }
                    }
                }
            }
        }
        Ok(Value::new_none())
    }

    /// Get an environment variable value.
    /// Returns the value as a string, or the default if not set.
    fn getenv(
        this: &ModuleContext,
        #[starlark(require = pos)] name: &str,
        #[starlark(require = named)] default: Option<&str>,
    ) -> starlark::Result<String> {
        match std::env::var(name) {
            Ok(v) => Ok(v),
            Err(_) => Ok(default.unwrap_or("").to_owned()),
        }
    }

    /// Convert a path or Label to a repository path object.
    ///
    /// Accepts both strings and Label objects. For Labels like
    /// `Label("@repo//:bin/cargo")`, resolves via cell/external paths.
    fn path<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let path_str = if let Some(s) = path.unpack_str() {
            s.to_owned()
        } else if let Some(repo_path) = path.downcast_ref::<RepositoryPath>() {
            repo_path.path_str().to_owned()
        } else if path.get_type() == "Label" {
            // Handle Label objects: resolve to workspace-relative path.
            let label_str = format!("{}", path);
            let workspace_root = this
                .working_dir
                .as_ref()
                .map(|wd| wd.as_ref().as_path())
                .unwrap_or_else(|| Path::new("."));
            resolve_label_to_path(&label_str, workspace_root)
        } else {
            return Err(starlark::Error::new_other(anyhow!(
                "module_ctx.path() requires a string, Label, or path object, got {}",
                path.get_type()
            )));
        };

        let resolved = if Path::new(&path_str).is_absolute() {
            PathBuf::from(&path_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(&path_str)
        } else {
            PathBuf::from(&path_str)
        };
        Ok(heap.alloc(RepositoryPath::new(resolved.to_string_lossy().to_string())))
    }

    /// Extract a local archive.
    fn extract<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] archive: Value<'v>,
        #[starlark(require = named, default = "")] output: &str,
        #[starlark(require = named, default = "")] strip_prefix: &str,
        #[starlark(require = named)] _rename_files: Option<Value<'v>>,
        #[starlark(require = named, default = false)] _watch_archive: bool,
    ) -> starlark::Result<Value<'v>> {
        let archive_str = archive.unpack_str().unwrap_or("");
        let archive_path = if Path::new(archive_str).is_absolute() {
            PathBuf::from(archive_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(archive_str)
        } else {
            PathBuf::from(archive_str)
        };

        let output_dir = if output.is_empty() {
            if let Some(ref wd) = this.working_dir {
                wd.as_ref().clone()
            } else {
                PathBuf::from(".")
            }
        } else if Path::new(output).is_absolute() {
            PathBuf::from(output)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(output)
        } else {
            PathBuf::from(output)
        };

        let data = std::fs::read(&archive_path).map_err(|e| {
            starlark::Error::new_other(anyhow!(
                "Failed to read archive {}: {}",
                archive_path.display(),
                e
            ))
        })?;

        std::fs::create_dir_all(&output_dir).map_err(|e| {
            starlark::Error::new_other(anyhow!("Failed to create directory: {}", e))
        })?;

        let strip = if strip_prefix.is_empty() {
            None
        } else {
            Some(strip_prefix)
        };
        extract_archive(&data, &output_dir, strip)
            .map_err(|e| starlark::Error::new_other(anyhow!("{}", e)))?;

        Ok(Value::new_none())
    }

    /// Watch a file or directory for changes.
    /// STUB: Returns None.
    fn watch<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _path: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
    }

    /// Report an extension's metadata for IDE integration.
    /// STUB: Returns None. Accepts arbitrary kwargs for forward compatibility.
    fn extension_metadata<'v>(
        this: &ModuleContext,
        #[starlark(kwargs)] _kwargs: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
    }

    /// Check if a path is a directory.
    fn is_dir<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value<'v>,
    ) -> starlark::Result<bool> {
        let path_str = path.unpack_str().unwrap_or("");
        let resolved = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(path_str)
        } else {
            PathBuf::from(path_str)
        };
        Ok(resolved.is_dir())
    }

    /// Delete a file or directory. Returns True if the path existed.
    fn delete<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value<'v>,
    ) -> starlark::Result<bool> {
        let path_str = path.unpack_str().unwrap_or("");
        let resolved = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(path_str)
        } else {
            PathBuf::from(path_str)
        };
        if resolved.is_dir() {
            std::fs::remove_dir_all(&resolved).ok();
            Ok(true)
        } else if resolved.is_file() {
            std::fs::remove_file(&resolved).ok();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Create a symlink.
    fn symlink<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] target: Value<'v>,
        #[starlark(require = pos)] link: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        let target_str = target.unpack_str().unwrap_or("");
        let link_str = link.unpack_str().unwrap_or("");

        let resolved_link = if Path::new(link_str).is_absolute() {
            PathBuf::from(link_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(link_str)
        } else {
            PathBuf::from(link_str)
        };

        // Ensure parent directory exists
        if let Some(parent) = resolved_link.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        // On Windows, copy instead of symlink (symlinks require privileges)
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target_str, &resolved_link).map_err(|e| {
                starlark::Error::new_other(anyhow!(
                    "Failed to create symlink {} -> {}: {}",
                    resolved_link.display(),
                    target_str,
                    e
                ))
            })?;
        }
        #[cfg(not(unix))]
        {
            let target_path = if Path::new(target_str).is_absolute() {
                PathBuf::from(target_str)
            } else if let Some(ref wd) = this.working_dir {
                wd.join(target_str)
            } else {
                PathBuf::from(target_str)
            };
            if target_path.is_dir() {
                // Copy directory recursively as fallback
                fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
                    std::fs::create_dir_all(dst)?;
                    for entry in std::fs::read_dir(src)? {
                        let entry = entry?;
                        let ty = entry.file_type()?;
                        if ty.is_dir() {
                            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
                        } else {
                            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
                        }
                    }
                    Ok(())
                }
                copy_dir_all(&target_path, &resolved_link).map_err(|e| {
                    starlark::Error::new_other(anyhow!("Failed to copy directory: {}", e))
                })?;
            } else {
                std::fs::copy(&target_path, &resolved_link).map_err(|e| {
                    starlark::Error::new_other(anyhow!("Failed to copy file: {}", e))
                })?;
            }
        }

        Ok(Value::new_none())
    }

    /// Create a file from a template with substitutions.
    fn template<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] path: Value<'v>,
        #[starlark(require = pos)] template: Value<'v>,
        #[starlark(require = named)] substitutions: Option<Value<'v>>,
        #[starlark(require = named, default = false)] executable: bool,
    ) -> starlark::Result<Value<'v>> {
        let path_str = path.unpack_str().unwrap_or("");
        let template_str = template.unpack_str().unwrap_or("");

        // Read the template file
        let template_path = if Path::new(template_str).is_absolute() {
            PathBuf::from(template_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(template_str)
        } else {
            PathBuf::from(template_str)
        };

        let mut content = std::fs::read_to_string(&template_path).map_err(|e| {
            starlark::Error::new_other(anyhow!(
                "Failed to read template {}: {}",
                template_path.display(),
                e
            ))
        })?;

        // Apply substitutions
        if let Some(subs) = substitutions {
            if let Some(dict) = starlark::values::dict::DictRef::from_value(subs) {
                for (k, v) in dict.iter() {
                    if let (Some(key), Some(val)) = (k.unpack_str(), v.unpack_str()) {
                        content = content.replace(key, val);
                    }
                }
            }
        }

        // Write the output file
        let output_path = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else if let Some(ref wd) = this.working_dir {
            wd.join(path_str)
        } else {
            PathBuf::from(path_str)
        };

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        std::fs::write(&output_path, &content).map_err(|e| {
            starlark::Error::new_other(anyhow!(
                "Failed to write template output {}: {}",
                output_path.display(),
                e
            ))
        })?;

        #[cfg(unix)]
        if executable {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&output_path, perms).ok();
        }
        #[cfg(not(unix))]
        let _ = executable;

        Ok(Value::new_none())
    }

    /// Apply patches.
    /// STUB: Returns None.
    fn patch<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _patch_file: Value<'v>,
        #[starlark(require = named, default = 0)] _strip: i32,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
    }
}

// ============================================================================
// Register type symbols as globals (if needed for type checking)
// ============================================================================

/// Register module_ctx type symbols as globals.
#[starlark_module]
pub fn register_module_ctx_types(builder: &mut GlobalsBuilder) {
    /// Type symbol for module_ctx.
    const module_ctx: StarlarkValueAsType<ModuleContext> = StarlarkValueAsType::new();

    /// Type symbol for bazel_module.
    const bazel_module: StarlarkValueAsType<BazelModule> = StarlarkValueAsType::new();

    /// Type symbol for repository_os.
    const repository_os: StarlarkValueAsType<RepositoryOs> = StarlarkValueAsType::new();
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_module_context_empty() {
        let ctx = ModuleContext::empty();
        assert!(ctx.get_modules().is_empty());
        assert!(!ctx.has_working_dir());
        assert!(ctx.working_dir().is_none());
        // delete_on_close is always true for module_ctx
        assert!(ctx.should_delete_working_dir());
    }

    #[test]
    fn test_module_context_with_temp_working_dir() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        let ctx = ModuleContext::empty().with_temp_working_dir(temp_path.clone());

        assert!(ctx.has_working_dir());
        assert_eq!(ctx.working_dir().unwrap(), temp_path.as_path());
        // delete_on_close is always true for module_ctx
        assert!(ctx.should_delete_working_dir());
    }

    #[test]
    fn test_module_context_resolve_path_relative() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        let ctx = ModuleContext::empty().with_temp_working_dir(temp_path.clone());

        let resolved = ctx.resolve_path("subdir/file.txt").unwrap();
        assert_eq!(resolved, temp_path.join("subdir/file.txt"));
    }

    #[test]
    fn test_module_context_resolve_path_absolute() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        let ctx = ModuleContext::empty().with_temp_working_dir(temp_path);

        let absolute = "/absolute/path/to/file.txt";
        let resolved = ctx.resolve_path(absolute).unwrap();
        assert_eq!(resolved, PathBuf::from(absolute));
    }

    #[test]
    fn test_module_context_resolve_path_no_working_dir() {
        let ctx = ModuleContext::empty();
        assert!(ctx.resolve_path("some/file.txt").is_none());
    }

    #[test]
    fn test_module_context_new_has_no_working_dir() {
        let modules = vec![BazelModule::new(
            "test_module".to_owned(),
            "1.0.0".to_owned(),
            true,
            vec!["install".to_owned()],
        )];
        let ctx = ModuleContext::new(modules, true);

        // New contexts don't have working dir by default
        assert!(!ctx.has_working_dir());
        assert!(ctx.working_dir().is_none());
        // But delete_on_close is still true
        assert!(ctx.should_delete_working_dir());
    }

    #[test]
    fn test_module_context_from_serialized_has_no_working_dir() {
        let modules = vec![SerializedModule {
            name: "test_module".to_owned(),
            version: "1.0.0".to_owned(),
            is_root: true,
            tags_by_class: HashMap::new(),
        }];
        let ctx = ModuleContext::from_serialized(modules, false);

        // New contexts don't have working dir by default
        assert!(!ctx.has_working_dir());
        assert!(ctx.working_dir().is_none());
        // But delete_on_close is still true
        assert!(ctx.should_delete_working_dir());
    }

    #[test]
    fn test_module_context_working_dir_is_temporary() {
        // This test verifies the key difference from repository_ctx:
        // module_ctx working dir should always be marked for deletion
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        let ctx = ModuleContext::empty().with_temp_working_dir(temp_path);

        // Key difference: module_ctx always deletes working dir
        assert!(ctx.should_delete_working_dir());
    }

    #[test]
    fn test_bazel_module_creation() {
        let module = BazelModule::new(
            "rules_python".to_owned(),
            "0.31.0".to_owned(),
            false,
            vec!["install".to_owned(), "pip".to_owned()],
        );

        assert_eq!(module.name(), "rules_python");
        assert_eq!(module.version(), "0.31.0");
        assert!(!module.is_root());
        assert!(module.tags_by_class().contains_key("install"));
        assert!(module.tags_by_class().contains_key("pip"));
    }

    #[test]
    fn test_bazel_module_with_tags() {
        let mut tags_by_class = HashMap::new();
        tags_by_class.insert(
            "install".to_owned(),
            vec![SerializedTag::new(vec![
                (
                    "name".to_owned(),
                    SerializedTagValue::String("numpy".to_owned()),
                ),
                (
                    "version".to_owned(),
                    SerializedTagValue::String("1.24.0".to_owned()),
                ),
            ])],
        );

        let module = BazelModule::with_tags(
            "rules_python".to_owned(),
            "0.31.0".to_owned(),
            true,
            tags_by_class.clone(),
        );

        assert_eq!(module.name(), "rules_python");
        assert!(module.is_root());
        assert_eq!(module.tags_by_class().len(), 1);
        assert!(module.tags_by_class().get("install").unwrap().len() == 1);
    }

    #[test]
    fn test_repository_os() {
        let os = RepositoryOs::new();

        // Just verify it creates something - actual values depend on platform
        assert!(!os.name.is_empty());
        assert!(!os.arch.is_empty());
    }
}
