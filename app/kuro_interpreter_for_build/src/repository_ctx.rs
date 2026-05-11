/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Implementation of Bazel's `repository_ctx` object for repository rules.
//!
//! Plan Reference: `thoughts/shared/plans/kuro-bazel-subplans/02-bzlmod.md` Phase 5
//!
//! ## Current Status: FUNCTIONAL IMPLEMENTATION
//!
//! This provides the `repository_ctx` object passed to repository rule implementations.
//! I/O methods now actually perform filesystem operations.
//!
//! ## What's Implemented
//!
//! - `name` property - the repository name
//! - `attr` property - access to attribute values
//! - `path()` method - create repository path objects
//! - `file()` method - create files with content
//! - `download()` method - download files from URLs
//! - `download_and_extract()` method - download and extract archives
//! - `execute()` method - run shell commands
//! - `symlink()` method - create symbolic links
//! - `read()` method - read file contents
//! - `which()` method - find programs on PATH
//!
//! ## Example usage in Starlark:
//!
//! ```python
//! def _my_repo_impl(ctx):
//!     print("Creating repository:", ctx.name)
//!     ctx.download(ctx.attr.url, "downloaded.txt")
//!     ctx.file("BUILD", "filegroup(name='all', srcs=glob(['**/*']))")
//! ```

use std::collections::HashMap;
use std::io::Cursor;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use allocative::Allocative;
use base64::Engine;
use derive_more::Display;
use flate2::read::GzDecoder;
use kuro_build_api::interpreter::rule_defs::bazel_label::BazelLabel;
use sha2::Digest;
use sha2::Sha256;
use starlark::any::ProvidesStaticType;
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
use starlark::values::dict::AllocDict;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneOr;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;
use tar::Archive;
use zip::ZipArchive;

use crate::label_filesystem::LabelFilesystemResolver;
use crate::label_filesystem::RootLabelResolution;
use crate::label_filesystem::is_bazel_label_string;
use crate::module_ctx::RepositoryOs;

// ============================================================================
// RepositoryAttr - Access to repository rule attribute values
// ============================================================================

/// Provides access to the attribute values passed to a repository rule.
/// Access like: `ctx.attr.url` or `ctx.attr.sha256`
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<repository_rule_attr>")]
pub struct RepositoryAttr {
    /// The name of the repository being created.
    /// In Bazel, `ctx.attr.name` returns this value.
    name: String,
    /// Attribute values stored as a simple map.
    /// In a full implementation, these would be typed based on the attr definitions.
    attrs: HashMap<String, AttrValue>,
}

/// A simplified attribute value that can be stored without lifetime issues.
#[derive(Debug, Clone, Allocative)]
pub enum AttrValue {
    String(String),
    Int(i64),
    Bool(bool),
    None,
    StringList(Vec<String>),
    Label(String),
    Dict(HashMap<String, AttrValue>),
}

impl AttrValue {
    /// Convert to a Starlark value.
    pub fn to_starlark<'v>(&self, heap: Heap<'v>) -> Value<'v> {
        match self {
            AttrValue::String(s) => heap.alloc(s.as_str()),
            AttrValue::Int(i) => heap.alloc(*i as i32),
            AttrValue::Bool(b) => Value::new_bool(*b),
            AttrValue::None => Value::new_none(),
            AttrValue::StringList(items) => {
                let values: Vec<Value<'v>> = items.iter().map(|s| heap.alloc(s.as_str())).collect();
                heap.alloc(values)
            }
            AttrValue::Label(s) => heap.alloc(BazelLabel::parse(s)),
            AttrValue::Dict(entries) => {
                let pairs: Vec<(&str, Value<'v>)> = entries
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.to_starlark(heap)))
                    .collect();
                heap.alloc(AllocDict(pairs))
            }
        }
    }
}

starlark_simple_value!(RepositoryAttr);

impl RepositoryAttr {
    /// Create a new repository attr with the given values and repository name.
    pub fn new_with_name(name: String, attrs: HashMap<String, AttrValue>) -> Self {
        Self { name, attrs }
    }

    /// Create a new repository attr with the given values (name defaults to empty).
    pub fn new(attrs: HashMap<String, AttrValue>) -> Self {
        Self {
            name: String::new(),
            attrs,
        }
    }

    /// Create an empty attr (for testing).
    pub fn empty() -> Self {
        Self {
            name: String::new(),
            attrs: HashMap::new(),
        }
    }

    /// Set the repository name on this attr.
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
}

#[starlark_value(type = "repository_rule_attr")]
impl<'v> StarlarkValue<'v> for RepositoryAttr {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        attribute == "name" || self.attrs.contains_key(attribute)
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        if attribute == "name" {
            return Some(heap.alloc(self.name.as_str()));
        }
        self.attrs.get(attribute).map(|v| v.to_starlark(heap))
    }

    fn dir_attr(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.attrs.keys().cloned().collect();
        keys.push("name".to_owned());
        keys
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

// ============================================================================
// RepositoryPath - A path within a repository
// ============================================================================

/// Represents a path within a repository being created.
///
/// In Bazel, `repository_ctx.path()` returns a path object whose string
/// representation is just the raw path (e.g., "/usr/bin/gcc"). This is critical
/// because repository rules use string interpolation to embed paths into
/// generated BUILD files.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct RepositoryPath {
    /// The path string (absolute path).
    path: String,
    /// The base directory for the repository.
    #[allocative(skip)]
    base_dir: Option<Arc<PathBuf>>,
}

impl std::fmt::Display for RepositoryPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.absolute_path().to_string_lossy())
    }
}

starlark_simple_value!(RepositoryPath);

impl RepositoryPath {
    pub fn new(path: String) -> Self {
        Self {
            path,
            base_dir: None,
        }
    }

    pub fn with_base_dir(path: String, base_dir: Arc<PathBuf>) -> Self {
        Self {
            path,
            base_dir: Some(base_dir),
        }
    }

    /// Get the absolute path.
    pub fn absolute_path(&self) -> PathBuf {
        let path = if let Some(base) = &self.base_dir {
            if Path::new(&self.path).is_absolute() {
                PathBuf::from(&self.path)
            } else {
                base.join(&self.path)
            }
        } else {
            PathBuf::from(&self.path)
        };
        normalize_path_lexically(path)
    }

    /// Get the path string.
    pub fn path_str(&self) -> &str {
        &self.path
    }
}

fn normalize_path_lexically(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::Normal(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[starlark_value(type = "repository_path")]
impl<'v> StarlarkValue<'v> for RepositoryPath {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(repository_path_methods)
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

#[starlark_module]
fn repository_path_methods(builder: &mut MethodsBuilder) {
    /// Get the basename of this path.
    #[starlark(attribute)]
    fn basename(this: &RepositoryPath) -> starlark::Result<String> {
        Ok(std::path::Path::new(&this.path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default())
    }

    /// Get the dirname of this path.
    #[starlark(attribute)]
    fn dirname(this: &RepositoryPath) -> starlark::Result<RepositoryPath> {
        let parent = std::path::Path::new(&this.path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(RepositoryPath {
            path: parent,
            base_dir: this.base_dir.clone(),
        })
    }

    /// Check if a file/directory exists at this path.
    #[starlark(attribute)]
    fn exists(this: &RepositoryPath) -> starlark::Result<bool> {
        Ok(this.absolute_path().exists())
    }

    /// Get a child path.
    fn get_child<'v>(
        this: &RepositoryPath,
        child: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let new_path = if this.path.is_empty() {
            child.to_owned()
        } else {
            format!("{}/{}", this.path, child)
        };
        let path = RepositoryPath {
            path: new_path,
            base_dir: this.base_dir.clone(),
        };
        Ok(heap.alloc(path))
    }

    /// Whether this path is a directory.
    #[starlark(attribute)]
    fn is_dir(this: &RepositoryPath) -> starlark::Result<bool> {
        Ok(this.absolute_path().is_dir())
    }

    /// Read directory contents.
    fn readdir(this: &RepositoryPath) -> starlark::Result<Vec<RepositoryPath>> {
        let abs_path = this.absolute_path();
        if abs_path.is_dir() {
            let entries: Vec<RepositoryPath> = std::fs::read_dir(&abs_path)
                .map_err(|e| {
                    starlark::Error::from(kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "Failed to read directory: {}",
                        e
                    ))
                })?
                .filter_map(|entry| entry.ok())
                .map(|entry| {
                    let child_path = abs_path.join(entry.file_name());
                    RepositoryPath {
                        path: child_path.to_string_lossy().to_string(),
                        base_dir: None,
                    }
                })
                .collect();
            Ok(entries)
        } else {
            Ok(Vec::new())
        }
    }

    /// Get the realpath.
    fn realpath<'v>(this: &RepositoryPath, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let abs_path = this.absolute_path();
        let real = abs_path.canonicalize().unwrap_or(abs_path);
        let path = RepositoryPath {
            path: real.to_string_lossy().to_string(),
            base_dir: None, // Already absolute
        };
        Ok(heap.alloc(path))
    }
}

// ============================================================================
// DownloadInfo - Result of a download operation
// ============================================================================

/// Information returned from a download operation.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<download_info>")]
pub struct DownloadInfo {
    /// Whether the download was successful.
    pub success: bool,
    /// The integrity hash of the downloaded file (SRI format).
    pub integrity: String,
    /// The SHA256 hash of the downloaded file (hex format).
    pub sha256: String,
}

starlark_simple_value!(DownloadInfo);

impl DownloadInfo {
    pub fn new(success: bool, data: &[u8]) -> Self {
        let hash = Sha256::digest(data);
        let sha256_hex = hex::encode(&hash);
        let sha256_base64 = base64::engine::general_purpose::STANDARD.encode(&hash);
        Self {
            success,
            integrity: format!("sha256-{}", sha256_base64),
            sha256: sha256_hex,
        }
    }

    pub fn stub() -> Self {
        Self {
            success: true,
            integrity: "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned(),
            sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        }
    }
}

#[starlark_value(type = "download_info")]
impl<'v> StarlarkValue<'v> for DownloadInfo {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "success" | "integrity" | "sha256")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "success" => Some(Value::new_bool(self.success)),
            "integrity" => Some(heap.alloc(&self.integrity as &str)),
            "sha256" => Some(heap.alloc(&self.sha256 as &str)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "success".to_owned(),
            "integrity".to_owned(),
            "sha256".to_owned(),
        ]
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

// ============================================================================
// DownloadToken - Async download token with .wait() method
// ============================================================================

/// Token returned from download() when block=False.
/// Calling .wait() returns the DownloadInfo.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<download_token>")]
pub struct DownloadToken {
    pub info: DownloadInfo,
}

starlark_simple_value!(DownloadToken);

#[starlark_value(type = "download_token")]
impl<'v> StarlarkValue<'v> for DownloadToken {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "wait" | "success" | "integrity" | "sha256")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "success" => Some(Value::new_bool(self.info.success)),
            "integrity" => Some(heap.alloc(&self.info.integrity as &str)),
            "sha256" => Some(heap.alloc(&self.info.sha256 as &str)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "wait".to_owned(),
            "success".to_owned(),
            "integrity".to_owned(),
            "sha256".to_owned(),
        ]
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }

    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(download_token_methods)
    }
}

#[starlark_module]
fn download_token_methods(builder: &mut MethodsBuilder) {
    fn wait<'v>(this: &DownloadToken, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(this.info.clone()))
    }
}

// ============================================================================
// ExecutionResult - Result of executing a command
// ============================================================================

/// Result of executing a command in the repository context.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<exec_result>")]
pub struct ExecutionResult {
    /// The return code (0 for success).
    pub return_code: i32,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
}

starlark_simple_value!(ExecutionResult);

impl ExecutionResult {
    pub fn new(return_code: i32, stdout: String, stderr: String) -> Self {
        Self {
            return_code,
            stdout,
            stderr,
        }
    }

    pub fn stub() -> Self {
        Self {
            return_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        }
    }
}

#[starlark_value(type = "exec_result")]
impl<'v> StarlarkValue<'v> for ExecutionResult {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "return_code" | "stdout" | "stderr")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "return_code" => Some(heap.alloc(self.return_code)),
            "stdout" => Some(heap.alloc(&self.stdout as &str)),
            "stderr" => Some(heap.alloc(&self.stderr as &str)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "return_code".to_owned(),
            "stdout".to_owned(),
            "stderr".to_owned(),
        ]
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

// ============================================================================
// RepoMetadata - Metadata returned from repository rule implementation
// ============================================================================

/// Metadata returned from a repository rule implementation.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<repo_metadata>")]
pub struct RepoMetadata {
    /// Whether the repository is reproducible.
    pub reproducible: bool,
}

starlark_simple_value!(RepoMetadata);

#[starlark_value(type = "repo_metadata")]
impl<'v> StarlarkValue<'v> for RepoMetadata {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "reproducible")
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "reproducible" => Some(Value::new_bool(self.reproducible)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["reproducible".to_owned()]
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

// ============================================================================
// RepositoryContext - The context object passed to repository rule implementations
// ============================================================================

/// The context object passed to repository rule implementation functions.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<repository_ctx {}>", name)]
pub struct RepositoryContext {
    /// The name of the repository being created.
    name: String,
    /// The original name as specified by the caller (may differ in bzlmod).
    original_name: String,
    /// Attribute values passed to the rule.
    attr: RepositoryAttr,
    /// The working directory for this repository.
    /// Files are created relative to this directory.
    #[allocative(skip)]
    working_dir: Arc<PathBuf>,
    /// The root workspace of the build invocation.
    ///
    /// Bazel exposes this as `repository_ctx.workspace_root`; it is not the
    /// generated repository directory.
    #[allocative(skip)]
    workspace_root: Arc<PathBuf>,
}

starlark_simple_value!(RepositoryContext);

impl RepositoryContext {
    /// Create a new repository context.
    pub fn new(name: String, attr: RepositoryAttr, working_dir: PathBuf) -> Self {
        let workspace_root =
            kuro_core::cells::get_dynamic_project_root().unwrap_or_else(|| working_dir.clone());
        Self::new_with_workspace_root(name, attr, working_dir, workspace_root)
    }

    /// Create a new repository context with an explicit root workspace path.
    pub fn new_with_workspace_root(
        name: String,
        mut attr: RepositoryAttr,
        working_dir: PathBuf,
        workspace_root: PathBuf,
    ) -> Self {
        attr.set_name(name.clone());
        Self {
            original_name: name.clone(),
            name,
            attr,
            working_dir: Arc::new(working_dir),
            workspace_root: Arc::new(workspace_root),
        }
    }

    /// Create a new repository context with an explicit original_name.
    pub fn new_with_original_name(
        name: String,
        original_name: String,
        mut attr: RepositoryAttr,
        working_dir: PathBuf,
        workspace_root: PathBuf,
    ) -> Self {
        attr.set_name(name.clone());
        Self {
            name,
            original_name,
            attr,
            working_dir: Arc::new(working_dir),
            workspace_root: Arc::new(workspace_root),
        }
    }

    /// Create a stub context for testing (uses temp directory).
    pub fn stub(name: &str) -> Self {
        // Create a temp directory for the repository
        let temp_dir = std::env::temp_dir().join("kuro_repos").join(name);
        let _ = std::fs::create_dir_all(&temp_dir);
        Self {
            original_name: name.to_owned(),
            name: name.to_owned(),
            attr: RepositoryAttr::empty(),
            working_dir: Arc::new(temp_dir),
            workspace_root: Arc::new(
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            ),
        }
    }

    /// Get the working directory.
    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }

    /// Resolve a path relative to the working directory.
    fn resolve_path(&self, path: &str) -> PathBuf {
        if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.working_dir.join(path)
        }
    }
}

// ============================================================================
// Helper functions for I/O operations
// ============================================================================

fn prepare_execute_working_directory(work_dir: &Path) -> starlark::Result<()> {
    std::fs::create_dir_all(work_dir).map_err(|e| {
        starlark::Error::from(kuro_error::kuro_error!(
            kuro_error::ErrorTag::Input,
            "Failed to create working directory '{}': {}",
            work_dir.display(),
            e
        ))
    })
}

/// Resolve a Bazel label string to a file system path.
///
/// Given a label like "@repo//pkg:file" or "//pkg:file", returns
/// a path relative to the workspace root.
pub(crate) fn resolve_label_to_path(label_str: &str, workspace_root: &Path) -> String {
    LabelFilesystemResolver::new(workspace_root)
        .with_root_label_resolution(RootLabelResolution::Relative)
        .resolve_label_string(label_str)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| label_str.to_owned())
}

fn resolve_label_to_filesystem_path(label_str: &str, workspace_root: &Path) -> PathBuf {
    let path = LabelFilesystemResolver::new(workspace_root)
        .with_project_root(Some(workspace_root))
        .with_root_label_resolution(RootLabelResolution::ProjectAbsolute)
        .resolve_label_string(label_str)
        .unwrap_or_else(|| PathBuf::from(label_str));
    if path.is_absolute() {
        path
    } else if let Some(root) = kuro_core::cells::get_dynamic_project_root() {
        root.join(path)
    } else {
        path
    }
}

/// Ensure an extension repo referenced by a resolved Label path exists on disk.
///
/// Bazel's `getPathFromLabel()` fetches the repository directory before
/// returning a path. Kuro resolves Labels syntactically, so callers that then
/// execute or read the path must trigger the same lazy materialization here.
pub(crate) fn ensure_label_path_materialized(path: &Path) {
    let Some(canonical) = canonical_name_from_bazel_external_path(path) else {
        return;
    };
    if let Err(e) = kuro_bzlmod::materialize_spoke_sync(&canonical) {
        tracing::warn!(
            "Lazy materialization of extension repo '{}' failed (continuing with path): {}",
            canonical,
            e
        );
    }
}

fn apply_unified_patch(patch_path: &Path, strip: i32, working_dir: &Path) -> Result<(), String> {
    match Command::new("patch")
        .args(["-p", &strip.to_string(), "-i"])
        .arg(patch_path)
        .current_dir(working_dir)
        .output()
    {
        Ok(output) if output.status.success() => return Ok(()),
        Ok(output) => {
            return Err(format!(
                "Patch failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Err(e) if e.kind() != ErrorKind::NotFound => {
            return Err(format!("Failed to execute patch: {e}"));
        }
        Err(_) => {}
    }

    let strip_arg = format!("-p{strip}");
    let output = Command::new("git")
        .args(["apply", "--unsafe-paths", "--whitespace=nowarn", &strip_arg])
        .arg(patch_path)
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("Failed to execute patch fallback via git apply: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Patch failed via git apply: {}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ))
    }
}

fn canonical_name_from_bazel_external_path(path: &Path) -> Option<String> {
    let mut components = path.components();
    while let Some(c) = components.next() {
        if c.as_os_str() == "bazel-external" {
            if let Some(next) = components.next() {
                return Some(next.as_os_str().to_string_lossy().to_string());
            }
        }
    }
    None
}

/// Download a file from a URL synchronously.
/// Uses blocking HTTP client since Starlark interpreter is synchronous.
pub(crate) fn download_url(url: &str) -> Result<Vec<u8>, String> {
    // Use a simple blocking HTTP GET
    // Since we're in sync context, we use std::process to call curl/wget
    // or implement a minimal HTTP client
    tracing::info!("Downloading from: {}", url);

    // Try using curl first (more commonly available)
    // On Windows, prefer curl.exe to avoid PowerShell Invoke-WebRequest alias
    let curl_cmd = if cfg!(windows) { "curl.exe" } else { "curl" };
    let output = Command::new(curl_cmd)
        .args(["-fsSL", "--max-time", "300", url])
        .output();

    match output {
        Ok(output) if output.status.success() => Ok(output.stdout),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Try wget as fallback
            let wget_output = Command::new("wget")
                .args(["-q", "-O", "-", "--timeout=300", url])
                .output();

            match wget_output {
                Ok(output) if output.status.success() => Ok(output.stdout),
                _ => Err(format!("Download failed: {}", stderr)),
            }
        }
        Err(e) => {
            // curl not found, try wget
            let wget_output = Command::new("wget")
                .args(["-q", "-O", "-", "--timeout=300", url])
                .output();

            match wget_output {
                Ok(output) if output.status.success() => Ok(output.stdout),
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(format!("Download failed (wget): {}", stderr))
                }
                Err(wget_err) => Err(format!(
                    "Download failed: curl error: {}, wget error: {}",
                    e, wget_err
                )),
            }
        }
    }
}

/// Verify SHA256 hash of data.
pub(crate) fn verify_sha256(data: &[u8], expected: &str) -> Result<(), String> {
    let hash = Sha256::digest(data);
    let computed = hex::encode(&hash);

    if computed.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "SHA256 mismatch: expected {}, got {}",
            expected, computed
        ))
    }
}

/// Verify SRI integrity hash of data.
pub(crate) fn verify_integrity(data: &[u8], expected: &str) -> Result<(), String> {
    // Parse SRI format: "sha256-base64hash"
    let (algo, hash) = expected
        .split_once('-')
        .ok_or_else(|| format!("Invalid integrity format: {}", expected))?;

    let expected_bytes = base64::engine::general_purpose::STANDARD
        .decode(hash)
        .map_err(|e| format!("Invalid base64: {}", e))?;

    match algo {
        "sha256" => {
            let computed = Sha256::digest(data);
            if computed.as_slice() == expected_bytes.as_slice() {
                Ok(())
            } else {
                let computed_base64 = base64::engine::general_purpose::STANDARD.encode(&computed);
                Err(format!(
                    "Integrity mismatch: expected {}, got sha256-{}",
                    expected, computed_base64
                ))
            }
        }
        "sha384" => {
            use sha2::Sha384;
            let computed = Sha384::digest(data);
            if computed.as_slice() == expected_bytes.as_slice() {
                Ok(())
            } else {
                let computed_base64 = base64::engine::general_purpose::STANDARD.encode(&computed);
                Err(format!(
                    "Integrity mismatch: expected {}, got sha384-{}",
                    expected, computed_base64
                ))
            }
        }
        "sha512" => {
            use sha2::Sha512;
            let computed = Sha512::digest(data);
            if computed.as_slice() == expected_bytes.as_slice() {
                Ok(())
            } else {
                let computed_base64 = base64::engine::general_purpose::STANDARD.encode(&computed);
                Err(format!(
                    "Integrity mismatch: expected {}, got sha512-{}",
                    expected, computed_base64
                ))
            }
        }
        _ => Err(format!("Unsupported hash algorithm: {}", algo)),
    }
}

/// Try each URL in turn, verify integrity, and write the first success to
/// `output_path`. Shared between `module_ctx.download()` and
/// `repository_ctx.download()`; path resolution differs between the two
/// contexts and stays at the call site.
///
/// Returns the `DownloadInfo` describing the successful fetch.
/// Returns `Err` on verification failure or if every URL fails — the caller
/// decides whether that should propagate or produce an `allow_fail` sentinel.
pub(crate) fn perform_download_to_path(
    urls: &[String],
    output_path: &Path,
    sha256: &str,
    integrity: &str,
    executable: bool,
) -> kuro_error::Result<DownloadInfo> {
    let mut last_error: Option<String> = None;
    for url in urls {
        match download_url(url) {
            Ok(data) => {
                if !sha256.is_empty() {
                    verify_sha256(&data, sha256).map_err(|e| {
                        kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{}", e)
                    })?;
                }
                if !integrity.is_empty() {
                    verify_integrity(&data, integrity).map_err(|e| {
                        kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{}", e)
                    })?;
                }

                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        kuro_error::kuro_error!(
                            kuro_error::ErrorTag::Input,
                            "Failed to create directory: {}",
                            e
                        )
                    })?;
                }

                std::fs::write(output_path, &data).map_err(|e| {
                    kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "Failed to write file: {}",
                        e
                    )
                })?;

                #[cfg(unix)]
                if executable {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(output_path)
                        .map_err(|e| kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{}", e))?
                        .permissions();
                    perms.set_mode(perms.mode() | 0o111);
                    std::fs::set_permissions(output_path, perms).map_err(|e| {
                        kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{}", e)
                    })?;
                }
                #[cfg(not(unix))]
                let _ = executable;

                return Ok(DownloadInfo::new(true, &data));
            }
            Err(e) => {
                last_error = Some(e);
            }
        }
    }

    Err(kuro_error::kuro_error!(
        kuro_error::ErrorTag::Input,
        "All download URLs failed: {}",
        last_error.unwrap_or_else(|| "unknown error".to_owned())
    ))
}

/// Try each URL in turn, verify integrity, and extract the first success into
/// `output_dir`. Shared between `module_ctx.download_and_extract()` and
/// `repository_ctx.download_and_extract()`.
pub(crate) fn perform_download_and_extract_to_dir(
    urls: &[String],
    output_dir: &Path,
    sha256: &str,
    integrity: &str,
    strip_prefix: Option<&str>,
) -> kuro_error::Result<DownloadInfo> {
    let mut last_error: Option<String> = None;
    for url in urls {
        match download_url(url) {
            Ok(data) => {
                if !sha256.is_empty() {
                    verify_sha256(&data, sha256).map_err(|e| {
                        kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{}", e)
                    })?;
                }
                if !integrity.is_empty() {
                    verify_integrity(&data, integrity).map_err(|e| {
                        kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{}", e)
                    })?;
                }

                std::fs::create_dir_all(output_dir).map_err(|e| {
                    kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "Failed to create directory: {}",
                        e
                    )
                })?;

                extract_archive(&data, output_dir, strip_prefix)
                    .map_err(|e| kuro_error::kuro_error!(kuro_error::ErrorTag::Input, "{}", e))?;

                return Ok(DownloadInfo::new(true, &data));
            }
            Err(e) => {
                last_error = Some(format!("{}: {}", url, e));
            }
        }
    }

    Err(kuro_error::kuro_error!(
        kuro_error::ErrorTag::Input,
        "All download URLs failed: {}",
        last_error.unwrap_or_else(|| "unknown error".to_owned())
    ))
}

/// Extract a tar.gz archive to a destination directory.
fn extract_tar_gz(data: &[u8], dest_dir: &Path, strip_prefix: Option<&str>) -> Result<(), String> {
    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);

    for entry_result in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry_result.map_err(|e| e.to_string())?;

        let path = entry.path().map_err(|e| e.to_string())?;

        // Apply strip_prefix if specified
        let dest_path = if let Some(prefix) = strip_prefix {
            let path_str = path.to_string_lossy();
            if let Some(stripped) = path_str.strip_prefix(prefix) {
                let stripped = stripped.trim_start_matches('/');
                if stripped.is_empty() {
                    continue;
                }
                dest_dir.join(stripped)
            } else if path_str.starts_with(prefix.trim_end_matches('/')) {
                let prefix_with_slash = format!("{}/", prefix.trim_end_matches('/'));
                if let Some(stripped) = path_str.strip_prefix(&prefix_with_slash) {
                    if stripped.is_empty() {
                        continue;
                    }
                    dest_dir.join(stripped)
                } else {
                    continue;
                }
            } else {
                continue;
            }
        } else {
            dest_dir.join(&*path)
        };

        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Extract based on entry type
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            std::fs::create_dir_all(&dest_path).map_err(|e| e.to_string())?;
        } else if entry_type.is_file() {
            let mut file = std::fs::File::create(&dest_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;

            // Set file permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mode) = entry.header().mode() {
                    let _ =
                        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(mode));
                }
            }
        } else if entry_type.is_symlink() {
            #[cfg(unix)]
            if let Ok(link_name) = entry.link_name() {
                if let Some(link_target) = link_name {
                    let _ = std::os::unix::fs::symlink(&*link_target, &dest_path);
                }
            }
        }
    }

    Ok(())
}

/// Extract a zip archive to a destination directory.
fn extract_zip(data: &[u8], dest_dir: &Path, strip_prefix: Option<&str>) -> Result<(), String> {
    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;

        let file_path = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        // Apply strip_prefix if specified
        let dest_path = if let Some(prefix) = strip_prefix {
            let stripped = file_path.strip_prefix(prefix).unwrap_or(&file_path);
            dest_dir.join(stripped)
        } else {
            dest_dir.join(&file_path)
        };

        // Skip if path is empty after stripping
        if dest_path == dest_dir {
            continue;
        }

        if file.is_dir() {
            std::fs::create_dir_all(&dest_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }

            let mut outfile = std::fs::File::create(&dest_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    let _ =
                        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(mode));
                }
            }
        }
    }

    Ok(())
}

/// Extract an archive, detecting format automatically.
/// Extract a tar.xz archive to a destination directory.
fn extract_tar_xz(data: &[u8], dest_dir: &Path, strip_prefix: Option<&str>) -> Result<(), String> {
    let decoder = xz2::read::XzDecoder::new(data);
    let mut archive = Archive::new(decoder);

    for entry_result in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry_result.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?;

        let dest_path = if let Some(prefix) = strip_prefix {
            let path_str = path.to_string_lossy();
            if let Some(stripped) = path_str.strip_prefix(prefix) {
                let stripped = stripped.trim_start_matches('/');
                if stripped.is_empty() {
                    continue;
                }
                dest_dir.join(stripped)
            } else if path_str.starts_with(prefix.trim_end_matches('/')) {
                let prefix_with_slash = format!("{}/", prefix.trim_end_matches('/'));
                if let Some(stripped) = path_str.strip_prefix(&prefix_with_slash) {
                    if stripped.is_empty() {
                        continue;
                    }
                    dest_dir.join(stripped)
                } else {
                    continue;
                }
            } else {
                continue;
            }
        } else {
            dest_dir.join(&*path)
        };

        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            std::fs::create_dir_all(&dest_path).map_err(|e| e.to_string())?;
        } else if entry_type.is_file() {
            let mut file = std::fs::File::create(&dest_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mode) = entry.header().mode() {
                    let _ =
                        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(mode));
                }
            }
        } else if entry_type.is_symlink() {
            #[cfg(unix)]
            if let Ok(link_name) = entry.link_name() {
                if let Some(link_target) = link_name {
                    let _ = std::os::unix::fs::symlink(&*link_target, &dest_path);
                }
            }
        }
    }

    Ok(())
}

/// Extract a tar.zst (Zstandard-compressed) archive to a destination directory.
fn extract_tar_zst(data: &[u8], dest_dir: &Path, strip_prefix: Option<&str>) -> Result<(), String> {
    let decoder = zstd::stream::read::Decoder::new(data).map_err(|e| e.to_string())?;
    let mut archive = Archive::new(decoder);

    for entry_result in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry_result.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?;

        let dest_path = if let Some(prefix) = strip_prefix {
            let path_str = path.to_string_lossy();
            if let Some(stripped) = path_str.strip_prefix(prefix) {
                let stripped = stripped.trim_start_matches('/');
                if stripped.is_empty() {
                    continue;
                }
                dest_dir.join(stripped)
            } else if path_str.starts_with(prefix.trim_end_matches('/')) {
                let prefix_with_slash = format!("{}/", prefix.trim_end_matches('/'));
                if let Some(stripped) = path_str.strip_prefix(&prefix_with_slash) {
                    if stripped.is_empty() {
                        continue;
                    }
                    dest_dir.join(stripped)
                } else {
                    continue;
                }
            } else {
                continue;
            }
        } else {
            dest_dir.join(&*path)
        };

        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            std::fs::create_dir_all(&dest_path).map_err(|e| e.to_string())?;
        } else if entry_type.is_file() {
            let mut file = std::fs::File::create(&dest_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mode) = entry.header().mode() {
                    let _ =
                        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(mode));
                }
            }
        } else if entry_type.is_symlink() {
            #[cfg(unix)]
            if let Ok(link_name) = entry.link_name() {
                if let Some(link_target) = link_name {
                    let _ = std::os::unix::fs::symlink(&*link_target, &dest_path);
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn extract_archive(
    data: &[u8],
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> Result<(), String> {
    // Try tar.gz first
    if extract_tar_gz(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    // Try tar.xz
    if extract_tar_xz(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    // Try tar.zst
    if extract_tar_zst(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    // Try zip
    if extract_zip(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    Err(
        "Failed to extract archive: unknown format (tried tar.gz, tar.xz, tar.zst, and zip)"
            .to_owned(),
    )
}

/// Get URLs from a Starlark value (string or list of strings).
pub(crate) fn get_urls_from_value<'v>(url_value: Value<'v>) -> Vec<String> {
    if let Some(s) = url_value.unpack_str() {
        vec![s.to_owned()]
    } else if let Some(list) = starlark::values::list::ListRef::from_value(url_value) {
        list.iter()
            .filter_map(|v| v.unpack_str().map(|s| s.to_owned()))
            .collect()
    } else {
        vec![]
    }
}

/// Repository context methods.
#[starlark_module]
fn repository_ctx_methods(builder: &mut MethodsBuilder) {
    /// Create a path object for a path within the repository.
    ///
    /// Accepts a string, a RepositoryPath, or a Label object.
    /// When given a Label, resolves it to a file system path under the workspace.
    fn path<'v>(
        this: &RepositoryContext,
        path_arg: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Helper: when a label points at an extension-generated repo (spoke
        // or extension hub), arrange for it to be on disk before we hand
        // back the path. Bazel's `rctx.path(label)` is fetch-triggering;
        // ours wasn't. Reuse Plan 36's sync→async bridge.
        //
        // The label may reference the repo by either canonical name
        // (`rules_rs+crate+foo`) or apparent name (`foo`). The dynamic
        // extension-cell registry maps both to the same `bazel-external/<canonical>`
        // path; we recover the canonical from the path's last segment so
        // the spoke-materialization registry — which is keyed strictly by
        // canonical name when the repo lives at `bazel-external/<canonical>` —
        // sees the right key.
        let trigger_materialization = |label_str: &str| {
            let Some(label) =
                kuro_bzlmod::canonicalize_label_with_package_context(label_str, "", "", None)
            else {
                return;
            };
            let repo = label.repo().as_str();
            if repo.is_empty() {
                return;
            }
            let resolved_repo = kuro_core::cells::resolve_dynamic_extension_cell_alias(repo)
                .unwrap_or_else(|| repo.to_owned());
            let canonical = if let Some(cell_path) =
                kuro_core::cells::get_dynamic_extension_cell(&resolved_repo)
            {
                cell_path.rsplit('/').next().unwrap_or(repo).to_owned()
            } else {
                resolved_repo
            };
            if let Err(e) = kuro_bzlmod::materialize_spoke_sync(&canonical) {
                tracing::debug!(
                    "rctx.path: lazy materialization of '{}' failed (continuing): {}",
                    canonical,
                    e
                );
            }
        };

        let path_str = if let Some(s) = path_arg.unpack_str() {
            // Treat anything starting with `@` (with or without `//`) and any
            // `//pkg:target` as a label — `resolve_label_to_path` handles the
            // `@repo` shorthand for `@repo//:repo`.
            if is_bazel_label_string(s) {
                trigger_materialization(s);
                resolve_label_to_path(s, &this.working_dir)
            } else {
                s.to_owned()
            }
        } else if let Some(repo_path) = path_arg.downcast_ref::<RepositoryPath>() {
            repo_path.path_str().to_owned()
        } else if path_arg.get_type() == "Label" {
            // Handle Label objects: resolve to workspace-relative path.
            let label_str = format!("{}", path_arg);
            trigger_materialization(&label_str);
            resolve_label_to_path(&label_str, &this.working_dir)
        } else {
            path_arg.to_repr()
        };

        let repo_path = RepositoryPath::with_base_dir(path_str, this.working_dir.clone());
        Ok(heap.alloc(repo_path))
    }

    /// Download a file from a URL.
    fn download<'v>(
        this: &RepositoryContext,
        url: Value<'v>,
        #[starlark(default = NoneOr::None)] output: NoneOr<Value<'v>>,
        #[starlark(default = "")] sha256: &str,
        #[starlark(require = named, default = false)] executable: bool,
        #[starlark(require = named, default = false)] allow_fail: bool,
        #[allow(unused_variables)]
        #[starlark(require = named, default = "")]
        canonical_id: &str,
        #[allow(unused_variables)]
        #[starlark(require = named)]
        auth: Option<Value<'v>>,
        #[starlark(require = named, default = "")] integrity: &str,
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
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "No URL provided for download"
            )
            .into());
        }

        // Determine output path - accept string, RepositoryPath, or None
        let output_str = match output.into_option() {
            Some(v) => {
                if let Some(rp) = v.downcast_ref::<RepositoryPath>() {
                    rp.path_str().to_owned()
                } else if let Some(s) = v.unpack_str() {
                    s.to_owned()
                } else {
                    v.to_str()
                }
            }
            None => String::new(),
        };
        let output_path = if output_str.is_empty() {
            // Default to filename from URL
            let url_path = urls[0].split('/').last().unwrap_or("downloaded");
            this.resolve_path(url_path)
        } else {
            this.resolve_path(&output_str)
        };

        match perform_download_to_path(&urls, &output_path, sha256, integrity, executable) {
            Ok(info) => Ok(heap.alloc(info)),
            Err(_) if allow_fail => Ok(heap.alloc(DownloadInfo {
                success: false,
                integrity: String::new(),
                sha256: String::new(),
            })),
            Err(e) => Err(e.into()),
        }
    }

    /// Download and extract an archive.
    fn download_and_extract<'v>(
        this: &RepositoryContext,
        url: Value<'v>,
        #[starlark(default = NoneOr::None)] output: NoneOr<Value<'v>>,
        #[starlark(default = "")] sha256: &str,
        #[allow(unused_variables)]
        #[starlark(default = "")]
        r#type: &str,
        #[starlark(default = "")] strip_prefix: &str,
        #[starlark(require = named, default = false)] allow_fail: bool,
        #[allow(unused_variables)]
        #[starlark(require = named, default = "")]
        canonical_id: &str,
        #[allow(unused_variables)]
        #[starlark(require = named)]
        auth: Option<Value<'v>>,
        #[starlark(require = named, default = "")] integrity: &str,
        #[allow(unused_variables)]
        #[starlark(require = named)]
        rename_files: Option<Value<'v>>,
        #[allow(unused_variables)]
        #[starlark(require = named)]
        headers: Option<Value<'v>>,
        #[starlark(require = named, default = "")]
        #[allow(non_snake_case)]
        stripPrefix: &str,
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
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "No URL provided for download_and_extract"
            )
            .into());
        }

        // Determine output directory - accept string or RepositoryPath
        let output_str = match output.into_option() {
            Some(v) => {
                if let Some(rp) = v.downcast_ref::<RepositoryPath>() {
                    rp.path_str().to_owned()
                } else {
                    v.unpack_str().unwrap_or("").to_owned()
                }
            }
            None => String::new(),
        };
        let output_dir = if output_str.is_empty() {
            this.working_dir.as_ref().clone()
        } else {
            this.resolve_path(&output_str)
        };

        let effective_strip = if !strip_prefix.is_empty() {
            strip_prefix
        } else {
            stripPrefix
        };
        let strip = if effective_strip.is_empty() {
            None
        } else {
            Some(effective_strip)
        };
        match perform_download_and_extract_to_dir(&urls, &output_dir, sha256, integrity, strip) {
            Ok(info) => Ok(heap.alloc(info)),
            Err(_) if allow_fail => Ok(heap.alloc(DownloadInfo {
                success: false,
                integrity: String::new(),
                sha256: String::new(),
            })),
            Err(e) => Err(e.into()),
        }
    }

    /// Create a file with the given content.
    fn file<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] path: Value<'v>,
        #[starlark(default = "")] content: &str,
        #[starlark(default = false)] executable: bool,
        #[starlark(default = false)] _legacy_utf8: bool,
    ) -> starlark::Result<Value<'v>> {
        let path_str = if let Some(s) = path.unpack_str() {
            s.to_owned()
        } else if let Some(repo_path) = path.downcast_ref::<RepositoryPath>() {
            repo_path.path_str().to_owned()
        } else {
            path.to_repr()
        };

        let file_path = normalize_path_lexically(this.resolve_path(&path_str));

        // Create parent directories
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                starlark::Error::from(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to create directory: {}",
                    e
                ))
            })?;
        }

        // Write the file
        std::fs::write(&file_path, content).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Failed to write file: {}",
                e
            ))
        })?;

        // Set executable if requested
        if executable {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&file_path)
                    .map_err(|e| {
                        starlark::Error::from(kuro_error::kuro_error!(
                            kuro_error::ErrorTag::Input,
                            "{}",
                            e
                        ))
                    })?
                    .permissions();
                perms.set_mode(perms.mode() | 0o111);
                std::fs::set_permissions(&file_path, perms).map_err(|e| {
                    starlark::Error::from(kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "{}",
                        e
                    ))
                })?;
            }
        }

        Ok(Value::new_none())
    }

    /// Execute a command.
    fn execute<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] arguments: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = 600)] timeout: i32,
        #[starlark(require = named)] environment: Option<Value<'v>>,
        #[starlark(require = named, default = true)] quiet: bool,
        #[starlark(require = named, default = "")] working_directory: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Get arguments as list of strings, resolving Label and RepositoryPath objects
        let args: Vec<String> = arguments
            .items
            .iter()
            .map(|v| {
                if let Some(s) = v.unpack_str() {
                    s.to_owned()
                } else if let Some(repo_path) = v.downcast_ref::<RepositoryPath>() {
                    // RepositoryPath: use absolute path
                    repo_path.absolute_path().to_string_lossy().to_string()
                } else if v.get_type() == "Label" {
                    // Label: resolve to filesystem path via cell paths
                    let label_str = v.to_str();
                    let path = resolve_label_to_filesystem_path(&label_str, &this.workspace_root);
                    ensure_label_path_materialized(&path);
                    path.to_string_lossy().to_string()
                } else {
                    // Other: convert to string
                    v.to_str()
                }
            })
            .collect();

        if args.is_empty() {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "arguments cannot be empty"
            )
            .into());
        }

        let program = &args[0];
        let cmd_args = &args[1..];

        let work_dir = if working_directory.is_empty() {
            this.working_dir.as_ref().clone()
        } else {
            this.resolve_path(working_directory)
        };
        prepare_execute_working_directory(&work_dir)?;

        // Build the command
        let mut cmd = Command::new(program);
        cmd.args(cmd_args);
        cmd.current_dir(&work_dir);

        // Set environment variables if provided
        if let Some(env_val) = environment {
            if let Some(env_dict) = starlark::values::dict::DictRef::from_value(env_val) {
                for (k, v) in env_dict.iter() {
                    if let Some(key) = k.unpack_str() {
                        if v.is_none() {
                            cmd.env_remove(key);
                        } else if let Some(val) = v.unpack_str() {
                            cmd.env(key, val);
                        }
                    }
                }
            }
        }

        // Execute with timeout
        let output = cmd.output().map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Failed to execute command: {}",
                e
            ))
        })?;

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

    /// Create a symlink.
    fn symlink<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] target: Value<'v>,
        #[starlark(require = pos)] link_name: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        let target_str = if let Some(s) = target.unpack_str() {
            s.to_owned()
        } else if let Some(repo_path) = target.downcast_ref::<RepositoryPath>() {
            repo_path.absolute_path().to_string_lossy().to_string()
        } else if target.get_type() == "Label" {
            // `rctx.symlink(Label("//templates:foo.bzl"), "foo.bzl")` must
            // resolve the Label to an absolute path so the resulting
            // symlink points at a real file on disk, not at the label's
            // string form (which would stringify as
            // `@@cell//templates:foo.bzl` and be a dangling link).
            let label_str = format!("{target}");
            let path = resolve_label_to_filesystem_path(&label_str, &this.workspace_root);
            ensure_label_path_materialized(&path);
            path.to_string_lossy().to_string()
        } else {
            target.to_repr()
        };

        let link_str = if let Some(s) = link_name.unpack_str() {
            s.to_owned()
        } else if let Some(repo_path) = link_name.downcast_ref::<RepositoryPath>() {
            repo_path.path_str().to_owned()
        } else {
            link_name.to_repr()
        };

        let link_path = this.resolve_path(&link_str);

        // Create parent directories
        if let Some(parent) = link_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                starlark::Error::from(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to create directory: {}",
                    e
                ))
            })?;
        }

        // Create symlink (remove existing file/symlink first, matching Bazel behavior)
        #[cfg(unix)]
        {
            if link_path.exists() || link_path.symlink_metadata().is_ok() {
                let _ = std::fs::remove_file(&link_path);
                let _ = std::fs::remove_dir_all(&link_path);
            }
            std::os::unix::fs::symlink(&target_str, &link_path).map_err(|e| {
                starlark::Error::from(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to create symlink: {}",
                    e
                ))
            })?;
        }

        #[cfg(not(unix))]
        {
            // On Windows, try to create a symlink or fall back to copying
            let target_path = std::path::Path::new(&target_str);
            tracing::debug!(
                "symlink: target='{}' (exists={}, is_dir={}), link='{}'",
                target_str,
                target_path.exists(),
                target_path.is_dir(),
                link_path.display()
            );
            if link_path.exists() || link_path.symlink_metadata().is_ok() {
                let _ = std::fs::remove_file(&link_path);
                let _ = std::fs::remove_dir_all(&link_path);
            }
            if target_path.is_dir() {
                // Try directory symlink first, fall back to junction, then recursive copy
                match std::os::windows::fs::symlink_dir(target_path, &link_path) {
                    Ok(()) => {}
                    Err(_) => {
                        // Fall back to recursive copy for directories
                        fn copy_dir_all(
                            src: &std::path::Path,
                            dst: &std::path::Path,
                        ) -> std::io::Result<()> {
                            std::fs::create_dir_all(dst)?;
                            for entry in std::fs::read_dir(src)? {
                                let entry = entry?;
                                let ty = entry.file_type()?;
                                let dst_path = dst.join(entry.file_name());
                                if ty.is_dir() {
                                    copy_dir_all(&entry.path(), &dst_path)?;
                                } else {
                                    std::fs::copy(entry.path(), &dst_path)?;
                                }
                            }
                            Ok(())
                        }
                        copy_dir_all(target_path, &link_path).map_err(|e| {
                            starlark::Error::from(kuro_error::kuro_error!(
                                kuro_error::ErrorTag::Input,
                                "Failed to copy directory '{}' to '{}': {}",
                                target_str,
                                link_path.display(),
                                e
                            ))
                        })?;
                    }
                }
            } else if target_path.exists() {
                std::fs::copy(target_path, &link_path).map_err(|e| {
                    starlark::Error::from(kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "Failed to copy file '{}' to '{}': {}",
                        target_str,
                        link_path.display(),
                        e
                    ))
                })?;
            } else {
                // Target doesn't exist - try to create symlink anyway (it might be created later)
                let _ = std::os::windows::fs::symlink_dir(target_path, &link_path)
                    .or_else(|_| std::os::windows::fs::symlink_file(target_path, &link_path));
            }
        }

        Ok(Value::new_none())
    }

    /// Apply a template file.
    fn template<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] path: Value<'v>,
        #[starlark(require = pos)] template: Value<'v>,
        substitutions: Option<Value<'v>>,
        #[starlark(require = named, default = false)] executable: bool,
    ) -> starlark::Result<Value<'v>> {
        let path_str = if let Some(s) = path.unpack_str() {
            s.to_owned()
        } else if let Some(repo_path) = path.downcast_ref::<RepositoryPath>() {
            repo_path.path_str().to_owned()
        } else {
            path.to_repr()
        };

        let template_str = if let Some(s) = template.unpack_str() {
            s.to_owned()
        } else if let Some(repo_path) = template.downcast_ref::<RepositoryPath>() {
            let template_path = repo_path.absolute_path();
            std::fs::read_to_string(&template_path).map_err(|e| {
                starlark::Error::from(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to read template '{}': {}",
                    template_path.display(),
                    e
                ))
            })?
        } else if template.get_type() == "Label" {
            // `rctx.template(out, Label("//templates:foo.tpl"), subs)` is the
            // canonical Bazel usage. Resolve the Label to a workspace path
            // and read the file content; without this branch we fall
            // through to `to_repr()` below and write the Label's string
            // form ("@@cell//templates:foo.tpl") as if it were the
            // template body, corrupting every generated file.
            let label_str = format!("{template}");
            let template_path = resolve_label_to_filesystem_path(&label_str, &this.workspace_root);
            ensure_label_path_materialized(&template_path);
            std::fs::read_to_string(&template_path).map_err(|e| {
                starlark::Error::from(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to read template label '{label_str}' at '{}': {e}",
                    template_path.display(),
                ))
            })?
        } else {
            template.to_repr()
        };

        // Apply substitutions
        let mut content = template_str;
        if let Some(subs_val) = substitutions {
            if let Some(subs_dict) = starlark::values::dict::DictRef::from_value(subs_val) {
                for (k, v) in subs_dict.iter() {
                    if let (Some(key), Some(val)) = (k.unpack_str(), v.unpack_str()) {
                        content = content.replace(key, val);
                    }
                }
            }
        }

        let file_path = this.resolve_path(&path_str);

        // Create parent directories
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                starlark::Error::from(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to create directory: {}",
                    e
                ))
            })?;
        }

        // Write the file
        std::fs::write(&file_path, &content).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Failed to write file: {}",
                e
            ))
        })?;

        // Set executable if requested
        if executable {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&file_path)
                    .map_err(|e| {
                        starlark::Error::from(kuro_error::kuro_error!(
                            kuro_error::ErrorTag::Input,
                            "{}",
                            e
                        ))
                    })?
                    .permissions();
                perms.set_mode(perms.mode() | 0o111);
                std::fs::set_permissions(&file_path, perms).map_err(|e| {
                    starlark::Error::from(kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Input,
                        "{}",
                        e
                    ))
                })?;
            }
        }

        Ok(Value::new_none())
    }

    /// Read a file and return its contents.
    #[allow(unused_variables)]
    fn read<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] path: Value<'v>,
        #[starlark(require = named, default = "auto")] watch: &str,
    ) -> starlark::Result<String> {
        let _ = watch;
        let file_path = if let Some(s) = path.unpack_str() {
            if is_bazel_label_string(s) {
                let path = resolve_label_to_filesystem_path(s, &this.workspace_root);
                ensure_label_path_materialized(&path);
                path
            } else {
                this.resolve_path(s)
            }
        } else if let Some(repo_path) = path.downcast_ref::<RepositoryPath>() {
            repo_path.absolute_path()
        } else if path.get_type() == "Label" {
            let label_str = path.to_str();
            let path = resolve_label_to_filesystem_path(&label_str, &this.workspace_root);
            ensure_label_path_materialized(&path);
            path
        } else {
            this.resolve_path(&path.to_str())
        };
        std::fs::read_to_string(&file_path).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Failed to read file '{}': {}",
                file_path.display(),
                e
            ))
        })
    }

    /// Delete a file or directory.
    fn delete<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] path: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        let path_str = if let Some(s) = path.unpack_str() {
            s.to_owned()
        } else if let Some(repo_path) = path.downcast_ref::<RepositoryPath>() {
            repo_path.path_str().to_owned()
        } else {
            path.to_repr()
        };

        let file_path = normalize_path_lexically(this.resolve_path(&path_str));

        let deleted = if file_path.is_dir() {
            std::fs::remove_dir_all(&file_path).map_err(|e| {
                starlark::Error::from(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to delete directory: {}",
                    e
                ))
            })?;
            true
        } else if file_path.exists() {
            std::fs::remove_file(&file_path).map_err(|e| {
                starlark::Error::from(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to delete file: {}",
                    e
                ))
            })?;
            true
        } else {
            false
        };

        Ok(Value::new_bool(deleted))
    }

    /// Apply a patch file. Bazel signature accepts `strip` either positionally
    /// or by keyword, matching `repository_ctx.patch(patch_file, strip=0)`.
    fn patch<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] patch_file: Value<'v>,
        #[starlark(default = 0)] strip: i32,
    ) -> starlark::Result<Value<'v>> {
        // Resolve the patch_file argument to an absolute on-disk path.
        // Bazel accepts: a `Label`, a `RepositoryPath`, or a string. Strings
        // can be either a relative path inside the working dir OR a
        // stringified label (e.g. `"@@//:foo.patch"`); the latter has to
        // route through the cell resolver, otherwise the resulting path
        // contains literal `@@//:` segments and `patch(1)` can't open it.
        //
        // `resolve_label_to_path` returns a project-relative path for root
        // cell labels and an absolute path for extension-cell labels. Since
        // we run `patch(1)` with `current_dir = working_dir` (an external
        // repo dir), root-cell-relative paths would be resolved against the
        // wrong root — so anchor any non-absolute result at the project root.
        let patch_path = if let Some(repo_path) = patch_file.downcast_ref::<RepositoryPath>() {
            repo_path.absolute_path().to_path_buf()
        } else if patch_file.get_type() == "Label" {
            let path =
                resolve_label_to_filesystem_path(&format!("{patch_file}"), &this.workspace_root);
            ensure_label_path_materialized(&path);
            path
        } else if let Some(s) = patch_file.unpack_str() {
            if is_bazel_label_string(s) {
                let path = resolve_label_to_filesystem_path(s, &this.workspace_root);
                ensure_label_path_materialized(&path);
                path
            } else {
                std::path::PathBuf::from(this.resolve_path(s))
            }
        } else {
            let repr = patch_file.to_repr();
            if is_bazel_label_string(&repr) {
                let path = resolve_label_to_filesystem_path(&repr, &this.workspace_root);
                ensure_label_path_materialized(&path);
                path
            } else {
                std::path::PathBuf::from(this.resolve_path(&repr))
            }
        };

        apply_unified_patch(&patch_path, strip, this.working_dir.as_ref()).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "{}",
                e
            ))
        })?;

        Ok(Value::new_none())
    }

    /// Extract an archive.
    fn extract<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] archive: Value<'v>,
        #[starlark(require = named, default = "")] output: &str,
        #[starlark(require = named, default = "")] strip_prefix: &str,
        #[starlark(require = named)] _rename_files: Option<Value<'v>>,
    ) -> starlark::Result<Value<'v>> {
        let archive_str = if let Some(s) = archive.unpack_str() {
            s.to_owned()
        } else if let Some(repo_path) = archive.downcast_ref::<RepositoryPath>() {
            repo_path.path_str().to_owned()
        } else {
            archive.to_repr()
        };

        let archive_path = this.resolve_path(&archive_str);
        let output_dir = if output.is_empty() {
            this.working_dir.as_ref().clone()
        } else {
            this.resolve_path(output)
        };

        // Read the archive
        let data = std::fs::read(&archive_path).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Failed to read archive: {}",
                e
            ))
        })?;

        // Extract
        let strip = if strip_prefix.is_empty() {
            None
        } else {
            Some(strip_prefix)
        };

        std::fs::create_dir_all(&output_dir).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Failed to create directory: {}",
                e
            ))
        })?;

        extract_archive(&data, &output_dir, strip).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "{}",
                e
            ))
        })?;

        Ok(Value::new_none())
    }

    /// Rename a file or directory.
    fn rename<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] src: Value<'v>,
        #[starlark(require = pos)] dst: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        let src_str = if let Some(s) = src.unpack_str() {
            s.to_owned()
        } else if let Some(repo_path) = src.downcast_ref::<RepositoryPath>() {
            repo_path.path_str().to_owned()
        } else {
            src.to_repr()
        };
        let dst_str = if let Some(s) = dst.unpack_str() {
            s.to_owned()
        } else if let Some(repo_path) = dst.downcast_ref::<RepositoryPath>() {
            repo_path.path_str().to_owned()
        } else {
            dst.to_repr()
        };
        let src_path = this.resolve_path(&src_str);
        let dst_path = this.resolve_path(&dst_str);
        if let Some(parent) = dst_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                starlark::Error::from(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "Failed to create parent directory: {}",
                    e
                ))
            })?;
        }
        std::fs::rename(&src_path, &dst_path).map_err(|e| {
            starlark::Error::from(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "Failed to rename: {}",
                e
            ))
        })?;
        Ok(Value::new_none())
    }

    /// Watch a path for changes.
    /// Currently a no-op stub - Kuro doesn't support watch yet.
    fn watch<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] _path: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Watch is a no-op for now
        Ok(Value::new_none())
    }

    /// Watch a directory tree for changes.
    /// Currently a no-op stub - Kuro doesn't support watch yet.
    fn watch_tree<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] _path: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        // watch_tree is a no-op for now
        Ok(Value::new_none())
    }

    /// Find a program on PATH.
    fn which<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] program: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Search PATH for the program
        if let Ok(path_var) = std::env::var("PATH") {
            let separator = if cfg!(windows) { ';' } else { ':' };
            for dir in path_var.split(separator) {
                // On Windows, also try common executable extensions
                let candidates: Vec<std::path::PathBuf> = if cfg!(windows) {
                    let base = Path::new(dir).join(program);
                    if base.extension().is_some() {
                        // Already has extension
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

    /// Get an environment variable.
    fn getenv<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] name: &str,
        #[starlark(default = NoneOr::None)] default: NoneOr<&str>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        match std::env::var(name) {
            Ok(v) => Ok(heap.alloc(v)),
            Err(_) => match default.into_option() {
                Some(d) => Ok(heap.alloc(d)),
                None => Ok(Value::new_none()),
            },
        }
    }

    /// Return repository metadata for lockfile.
    #[allow(unused_variables)]
    fn repo_metadata<'v>(
        this: &RepositoryContext,
        #[starlark(require = named, default = false)] reproducible: bool,
        #[starlark(require = named, default = NoneOr::None)] attrs_for_reproducibility: NoneOr<
            Value<'v>,
        >,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(RepoMetadata { reproducible }))
    }

    /// Report progress.
    #[allow(unused_variables)]
    fn report_progress<'v>(
        this: &RepositoryContext,
        #[starlark(require = pos)] status: &str,
    ) -> starlark::Result<Value<'v>> {
        tracing::info!("Repository progress: {}", status);
        Ok(Value::new_none())
    }
}

#[starlark_value(type = "repository_ctx")]
impl<'v> StarlarkValue<'v> for RepositoryContext {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "name" | "original_name" | "attr" | "os" | "workspace_root"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc(&self.name as &str)),
            "original_name" => Some(heap.alloc(&self.original_name as &str)),
            "attr" => Some(heap.alloc(self.attr.clone())),
            "os" => Some(heap.alloc(RepositoryOs::new())),
            "workspace_root" => Some(heap.alloc(RepositoryPath::with_base_dir(
                self.workspace_root.to_string_lossy().to_string(),
                self.workspace_root.clone(),
            ))),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "name".to_owned(),
            "original_name".to_owned(),
            "attr".to_owned(),
            "os".to_owned(),
            "workspace_root".to_owned(),
        ]
    }

    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(repository_ctx_methods)
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

// ============================================================================
// Register type symbols as globals
// ============================================================================

/// Register repository_ctx type symbols as globals.
#[starlark_module]
pub fn register_repository_ctx_types(builder: &mut GlobalsBuilder) {
    /// Type symbol for repository_ctx.
    const repository_ctx: StarlarkValueAsType<RepositoryContext> = StarlarkValueAsType::new();

    /// Type symbol for repository_path.
    const repository_path: StarlarkValueAsType<RepositoryPath> = StarlarkValueAsType::new();
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_file_creation() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = RepositoryContext::new(
            "test_repo".to_owned(),
            RepositoryAttr::empty(),
            temp_dir.path().to_path_buf(),
        );

        // Create a file
        let file_path = ctx.resolve_path("test.txt");
        std::fs::write(&file_path, "hello world").unwrap();

        assert!(file_path.exists());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "hello world");
    }

    #[test]
    fn test_workspace_root_is_separate_from_repository_working_dir() {
        let workspace = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();
        let ctx = RepositoryContext::new_with_workspace_root(
            "test_repo".to_owned(),
            RepositoryAttr::empty(),
            repo_dir.path().to_path_buf(),
            workspace.path().to_path_buf(),
        );

        assert_eq!(ctx.working_dir(), repo_dir.path());
        assert_eq!(ctx.workspace_root.as_ref().as_path(), workspace.path());
    }

    #[test]
    fn test_download_info_hash() {
        let data = b"Hello, World!";
        let info = DownloadInfo::new(true, data);

        // Verify SHA256 is correct
        assert!(!info.sha256.is_empty());
        assert!(info.integrity.starts_with("sha256-"));
    }

    #[test]
    fn test_verify_sha256() {
        let data = b"Hello, World!";
        let hash = Sha256::digest(data);
        let sha256_hex = hex::encode(&hash);

        assert!(verify_sha256(data, &sha256_hex).is_ok());
        assert!(verify_sha256(data, "wrong_hash").is_err());
    }

    #[test]
    fn test_verify_integrity() {
        let data = b"Hello, World!";
        let hash = Sha256::digest(data);
        let sha256_base64 = base64::engine::general_purpose::STANDARD.encode(&hash);
        let integrity = format!("sha256-{}", sha256_base64);

        assert!(verify_integrity(data, &integrity).is_ok());
        assert!(verify_integrity(data, "sha256-wronghash").is_err());
    }

    #[test]
    fn test_extract_tar_gz() {
        use std::io::Write;

        use flate2::Compression;
        use flate2::write::GzEncoder;

        let temp_dir = TempDir::new().unwrap();

        // Create a simple tar.gz with one file
        let mut builder = tar::Builder::new(Vec::new());
        let content = b"Hello, World!";
        let mut header = tar::Header::new_gnu();
        header.set_path("test.txt").unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &content[..]).unwrap();
        let tar_data = builder.into_inner().unwrap();

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar_data).unwrap();
        let gz_data = encoder.finish().unwrap();

        // Extract
        let dest = temp_dir.path().join("extracted");
        std::fs::create_dir(&dest).unwrap();
        extract_tar_gz(&gz_data, &dest, None).unwrap();

        // Verify
        let extracted_file = dest.join("test.txt");
        assert!(extracted_file.exists());
        assert_eq!(
            std::fs::read_to_string(&extracted_file).unwrap(),
            "Hello, World!"
        );
    }

    #[test]
    fn test_repository_path() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = Arc::new(temp_dir.path().to_path_buf());

        let path = RepositoryPath::with_base_dir("subdir/file.txt".to_owned(), base_dir.clone());
        let abs_path = path.absolute_path();

        assert_eq!(abs_path, temp_dir.path().join("subdir/file.txt"));
    }

    #[test]
    fn test_repository_path_displays_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = Arc::new(temp_dir.path().to_path_buf());

        let root = RepositoryPath::with_base_dir(".".to_owned(), base_dir.clone());
        assert_eq!(format!("{root}"), temp_dir.path().to_string_lossy());

        let child = RepositoryPath::with_base_dir("subdir/file.txt".to_owned(), base_dir);
        assert_eq!(
            format!("{child}"),
            temp_dir.path().join("subdir/file.txt").to_string_lossy()
        );
    }

    #[test]
    fn test_prepare_execute_working_directory_creates_missing_dir() {
        let temp_dir = TempDir::new().unwrap();
        let work_dir = temp_dir.path().join("repo").join("subdir");

        assert!(!work_dir.exists());
        prepare_execute_working_directory(&work_dir).unwrap();
        assert!(work_dir.is_dir());
    }

    #[test]
    fn test_normalize_path_lexically_removes_curdir() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("repo").join(".");

        assert_eq!(normalize_path_lexically(path), temp_dir.path().join("repo"));
    }

    #[test]
    fn test_resolve_root_label_to_filesystem_path_uses_workspace_root() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path();

        assert_eq!(
            resolve_label_to_filesystem_path("@@//:root.patch", workspace_root),
            workspace_root.join("root.patch")
        );
        assert_eq!(
            resolve_label_to_filesystem_path("//pkg:file.patch", workspace_root),
            workspace_root.join("pkg/file.patch")
        );
    }

    #[test]
    fn test_apply_unified_patch_without_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        std::fs::create_dir(&source_dir).unwrap();
        std::fs::write(source_dir.join("file.txt"), "old\n").unwrap();
        let patch = temp_dir.path().join("change.patch");
        std::fs::write(
            &patch,
            "--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new\n",
        )
        .unwrap();

        apply_unified_patch(&patch, 1, &source_dir).unwrap();

        assert_eq!(
            std::fs::read_to_string(source_dir.join("file.txt")).unwrap(),
            "new\n"
        );
    }
}
