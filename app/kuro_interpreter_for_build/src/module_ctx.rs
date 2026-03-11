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
//! ## Current Status: PARTIALLY IMPLEMENTED
//!
//! This provides the `module_ctx` object passed to module extension implementations.
//! The `modules` property returns real module data with tags populated from
//! kuro_bzlmod's aggregated extension data.
//!
//! ## What's Implemented
//!
//! - `modules` property - list of bazel_module objects with tag data
//! - `os` property - repository_os struct with name, arch
//! - `root_module_has_non_dev_dependency` property
//! - Basic stub methods for I/O operations (download, execute, etc.)
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
use std::sync::Arc;

use allocative::Allocative;
use derive_more::Display;
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
use starlark::values::dict::Dict;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;
use starlark::values::structs::AllocStruct;

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
            SerializedTagValue::Label(s) => heap.alloc(s.as_str()),
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
        let fields: SmallMap<&str, Value<'v>> = self
            .kwargs
            .iter()
            .map(|(k, v)| (k.as_str(), v.to_starlark(heap)))
            .collect();
        heap.alloc(AllocStruct(fields.into_iter()))
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
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        self.tags_by_class.contains_key(attribute)
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        self.tags_by_class.get(attribute).map(|tags| {
            let structs: Vec<Value<'v>> = tags
                .iter()
                .map(|tag| tag.to_starlark_struct(heap))
                .collect();
            heap.alloc(structs)
        })
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
        matches!(attribute, "name" | "version" | "is_root" | "tags")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc(&self.name as &str)),
            "version" => Some(heap.alloc(&self.version as &str)),
            "is_root" => Some(Value::new_bool(self.is_root)),
            "tags" => Some(heap.alloc(BazelModuleTags::with_tags(self.tags_by_class.clone()))),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "name".to_owned(),
            "version".to_owned(),
            "is_root".to_owned(),
            "tags".to_owned(),
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

/// Module context methods - stub implementations for most operations.
/// NOTE: All methods that would normally perform I/O operations return None or empty values.
/// Full implementation requires integration with the bzlmod extension execution engine.
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
    /// STUB: Returns empty string.
    fn read(
        this: &ModuleContext,
        #[starlark(require = pos)] _path: Value,
        #[starlark(require = named, default = "auto")] _watch: &str,
    ) -> starlark::Result<String> {
        Ok(String::new())
    }

    /// Write a file with the given content.
    /// STUB: Returns None.
    fn file<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _path: Value<'v>,
        #[starlark(require = named, default = "")] _content: &str,
        #[starlark(require = named, default = false)] _executable: bool,
        #[starlark(require = named, default = false)] _legacy_utf8: bool,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
    }

    /// Download a file from a URL.
    /// STUB: Returns None.
    fn download<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _url: Value<'v>,
        #[starlark(require = named, default = "")] _output: &str,
        #[starlark(require = named, default = "")] _sha256: &str,
        #[starlark(require = named, default = "")] _integrity: &str,
        #[starlark(require = named, default = false)] _executable: bool,
        #[starlark(require = named, default = true)] _allow_fail: bool,
        #[starlark(require = named, default = "")] _canonical_id: &str,
        #[starlark(require = named)] _auth: Option<Value<'v>>,
        #[starlark(require = named)] _headers: Option<Value<'v>>,
        #[starlark(require = named, default = 0)] _block: i32,
    ) -> starlark::Result<Value<'v>> {
        // TODO: Implement actual download
        Ok(Value::new_none())
    }

    /// Download and extract an archive from a URL.
    /// STUB: Returns None.
    fn download_and_extract<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _url: Value<'v>,
        #[starlark(require = named, default = "")] _output: &str,
        #[starlark(require = named, default = "")] _sha256: &str,
        #[starlark(require = named, default = "")] _integrity: &str,
        #[starlark(require = named, default = "")] _strip_prefix: &str,
        #[starlark(require = named, default = "")] _type: &str,
        #[starlark(require = named)] _rename_files: Option<Value<'v>>,
        #[starlark(require = named)] _auth: Option<Value<'v>>,
        #[starlark(require = named)] _headers: Option<Value<'v>>,
        #[starlark(require = named, default = "")] _canonical_id: &str,
    ) -> starlark::Result<Value<'v>> {
        // TODO: Implement actual download_and_extract
        Ok(Value::new_none())
    }

    /// Execute a command and return its output.
    /// STUB: Returns None.
    fn execute<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _arguments: Value<'v>,
        #[starlark(require = named, default = 600)] _timeout: i32,
        #[starlark(require = named)] _environment: Option<Value<'v>>,
        #[starlark(require = named, default = true)] _quiet: bool,
        #[starlark(require = named, default = "")] _working_directory: &str,
    ) -> starlark::Result<Value<'v>> {
        // TODO: Implement actual execute
        Ok(Value::new_none())
    }

    /// Find the path to a program on PATH.
    /// STUB: Returns None.
    fn which<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _program: &str,
    ) -> starlark::Result<Value<'v>> {
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

    /// Convert a path to a repository path object.
    /// STUB: Returns the path string.
    fn path(
        this: &ModuleContext,
        #[starlark(require = pos)] path: &str,
    ) -> starlark::Result<String> {
        Ok(path.to_owned())
    }

    /// Extract an archive.
    /// STUB: Returns None.
    fn extract<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _archive: Value<'v>,
        #[starlark(require = named, default = "")] _output: &str,
        #[starlark(require = named, default = "")] _strip_prefix: &str,
        #[starlark(require = named)] _rename_files: Option<Value<'v>>,
        #[starlark(require = named, default = false)] _watch_archive: bool,
    ) -> starlark::Result<Value<'v>> {
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
    /// STUB: Returns None.
    fn extension_metadata<'v>(
        this: &ModuleContext,
        #[starlark(require = named)] _root_module_direct_deps: Option<Value<'v>>,
        #[starlark(require = named)] _root_module_direct_dev_deps: Option<Value<'v>>,
        #[starlark(require = named, default = true)] _reproducible: bool,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
    }

    /// Check if a path is a directory.
    /// STUB: Returns false.
    fn is_dir<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _path: Value<'v>,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Delete a file or directory.
    /// STUB: Returns None.
    fn delete<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _path: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
    }

    /// Create a symlink.
    /// STUB: Returns None.
    fn symlink<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _target: Value<'v>,
        #[starlark(require = pos)] _link: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
    }

    /// Apply a template file.
    /// STUB: Returns None.
    fn template<'v>(
        this: &ModuleContext,
        #[starlark(require = pos)] _path: Value<'v>,
        #[starlark(require = pos)] _template: Value<'v>,
        #[starlark(require = named)] _substitutions: Option<Value<'v>>,
        #[starlark(require = named, default = false)] _executable: bool,
    ) -> starlark::Result<Value<'v>> {
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
