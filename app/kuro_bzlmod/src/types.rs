/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Data structures for bzlmod MODULE.bazel parsing.
//!
//! These types represent the parsed content of a MODULE.bazel file,
//! including the module declaration and its dependencies.

use allocative::Allocative;

use crate::version::Version;

/// Represents a parsed MODULE.bazel file.
///
/// This structure contains all the information declared in a MODULE.bazel file,
/// including the module's identity and its dependencies.
///
/// # Example MODULE.bazel
///
/// ```starlark
/// module(
///     name = "my_project",
///     version = "1.0.0",
///     compatibility_level = 1,
/// )
///
/// bazel_dep(name = "rules_cc", version = "0.0.9")
/// bazel_dep(name = "rules_rust", version = "0.40.0", dev_dependency = True)
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct Module {
    /// The module's name (from `module(name = "...")`).
    /// This is used as the canonical name for the module in the dependency graph.
    pub name: String,

    /// The module's version string (from `module(version = "...")`).
    /// Parsed according to Bazel's relaxed SemVer format.
    pub version: Version,

    /// The compatibility level (from `module(compatibility_level = N)`).
    /// Modules with different compatibility levels cannot coexist in the
    /// dependency graph unless explicitly allowed via `multiple_version_override`.
    /// Defaults to 0.
    pub compatibility_level: u32,

    /// List of direct dependencies declared via `bazel_dep()`.
    pub bazel_deps: Vec<BazelDep>,

    /// Override directives (local_path_override, archive_override, etc.)
    pub overrides: Vec<Override>,
}

impl Module {
    /// Creates a new Module with the given name and version.
    pub fn new(name: String, version: Version) -> Self {
        Self {
            name,
            version,
            compatibility_level: 0,
            bazel_deps: Vec::new(),
            overrides: Vec::new(),
        }
    }

    /// Creates a default/empty module for use when no MODULE.bazel exists.
    pub fn empty() -> Self {
        Self {
            name: String::new(),
            version: Version::empty(),
            compatibility_level: 0,
            bazel_deps: Vec::new(),
            overrides: Vec::new(),
        }
    }
}

/// A dependency declared via `bazel_dep()` in MODULE.bazel.
///
/// # Example
///
/// ```starlark
/// bazel_dep(
///     name = "rules_cc",
///     version = "0.0.9",
///     repo_name = "cc_rules",  # optional: use @cc_rules instead of @rules_cc
///     dev_dependency = True,   # optional: only needed for development
/// )
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct BazelDep {
    /// The module name to depend on.
    pub name: String,

    /// The minimum required version.
    pub version: Version,

    /// Optional repository name override.
    /// If set, this module can be referenced as `@repo_name` instead of `@name`.
    pub repo_name: Option<String>,

    /// Whether this is a dev-only dependency.
    /// Dev dependencies are not included in the published module.
    pub dev_dependency: bool,
}

impl BazelDep {
    /// Creates a new BazelDep with the given name and version.
    pub fn new(name: String, version: Version) -> Self {
        Self {
            name,
            version,
            repo_name: None,
            dev_dependency: false,
        }
    }

    /// Returns the repository name to use for this dependency.
    /// Uses `repo_name` if set, otherwise uses `name`.
    pub fn apparent_name(&self) -> &str {
        self.repo_name.as_deref().unwrap_or(&self.name)
    }
}

/// Override directives that bypass normal version resolution.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum Override {
    /// Use a local directory for a module instead of fetching from registry.
    LocalPath(LocalPathOverride),

    /// Use a specific version, ignoring what the graph requests.
    SingleVersion(SingleVersionOverride),

    /// Allow multiple versions of a module to coexist.
    MultipleVersion(MultipleVersionOverride),

    /// Fetch module from an archive URL.
    Archive(ArchiveOverride),

    /// Fetch module from a git repository.
    Git(GitOverride),
}

/// `local_path_override()` - use a local directory for a module.
///
/// # Example
///
/// ```starlark
/// local_path_override(
///     module_name = "my_local_module",
///     path = "../my-local-module",
/// )
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct LocalPathOverride {
    /// The module name this override applies to.
    pub module_name: String,

    /// The local filesystem path (relative to workspace root).
    pub path: String,
}

/// `single_version_override()` - force a specific version.
///
/// # Example
///
/// ```starlark
/// single_version_override(
///     module_name = "protobuf",
///     version = "3.19.2",
///     registry = "https://bcr.bazel.build",
/// )
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct SingleVersionOverride {
    /// The module name this override applies to.
    pub module_name: String,

    /// The exact version to use.
    pub version: Version,

    /// Optional alternative registry URL.
    pub registry: Option<String>,

    /// Patches to apply after fetching.
    pub patches: Vec<String>,

    /// Number of leading path components to strip from patch paths.
    pub patch_strip: u32,
}

/// `multiple_version_override()` - allow multiple versions to coexist.
///
/// # Example
///
/// ```starlark
/// multiple_version_override(
///     module_name = "protobuf",
///     versions = ["3.18.0", "3.19.0"],
/// )
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct MultipleVersionOverride {
    /// The module name this override applies to.
    pub module_name: String,

    /// The allowed versions.
    pub versions: Vec<Version>,

    /// Optional alternative registry URL.
    pub registry: Option<String>,
}

/// `archive_override()` - fetch module from an archive URL.
///
/// # Example
///
/// ```starlark
/// archive_override(
///     module_name = "rules_cc",
///     urls = ["https://example.com/rules_cc.tar.gz"],
///     integrity = "sha256-...",
///     strip_prefix = "rules_cc-main",
/// )
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ArchiveOverride {
    /// The module name this override applies to.
    pub module_name: String,

    /// URLs to download the archive from (tried in order).
    pub urls: Vec<String>,

    /// Subresource Integrity hash (e.g., "sha256-base64hash").
    pub integrity: Option<String>,

    /// Directory prefix to strip from archive.
    pub strip_prefix: Option<String>,

    /// Patches to apply after extraction.
    pub patches: Vec<String>,

    /// Number of leading path components to strip from patch paths.
    pub patch_strip: u32,
}

/// `git_override()` - fetch module from a git repository.
///
/// # Example
///
/// ```starlark
/// git_override(
///     module_name = "rules_rust",
///     remote = "https://github.com/example/rules_rust.git",
///     commit = "abc123...",
/// )
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct GitOverride {
    /// The module name this override applies to.
    pub module_name: String,

    /// Git remote URL.
    pub remote: String,

    /// Commit hash to checkout.
    pub commit: String,

    /// Optional shallow_since for faster clones.
    pub shallow_since: Option<String>,

    /// Patches to apply after checkout.
    pub patches: Vec<String>,

    /// Number of leading path components to strip from patch paths.
    pub patch_strip: u32,
}

/// The result of parsing a MODULE.bazel file.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ParsedModuleFile {
    /// The parsed module information.
    pub module: Module,

    /// Whether a `module()` directive was present.
    /// If false, the module has default/empty values.
    pub has_module_directive: bool,

    /// Extension usages from `use_extension()` calls.
    pub extension_usages: Vec<ExtensionUsage>,
}

// ============================================================================
// Module Extension Types (Phase 5)
// ============================================================================

/// A `use_extension()` call in MODULE.bazel with its associated tags.
///
/// # Example
///
/// ```starlark
/// pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
/// pip.parse(
///     hub_name = "pip",
///     python_version = "3.11",
///     requirements_lock = "//:requirements_lock.txt",
/// )
/// use_repo(pip, "pip")
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ExtensionUsage {
    /// The .bzl file containing the extension (e.g., "@rules_python//python/extensions:pip.bzl").
    pub extension_bzl_file: String,

    /// The name of the extension in the .bzl file (e.g., "pip").
    pub extension_name: String,

    /// Whether this is a dev-only extension usage.
    pub dev_dependency: bool,

    /// Isolate this extension usage from other modules (Bazel 6.3+).
    pub isolate: bool,

    /// Tags applied to this extension (e.g., `pip.parse(...)`).
    pub tags: Vec<ExtensionTag>,

    /// Repositories imported via `use_repo()` for this extension.
    pub imports: Vec<UseRepo>,
}

impl ExtensionUsage {
    /// Create a new extension usage.
    pub fn new(extension_bzl_file: String, extension_name: String) -> Self {
        Self {
            extension_bzl_file,
            extension_name,
            dev_dependency: false,
            isolate: false,
            tags: Vec::new(),
            imports: Vec::new(),
        }
    }

    /// Get a unique identifier for this extension.
    pub fn extension_id(&self) -> String {
        format!("{}%{}", self.extension_bzl_file, self.extension_name)
    }
}

/// A tag call on an extension (e.g., `pip.parse(...)`).
///
/// Tags are method calls on the extension proxy returned by `use_extension()`.
/// They provide configuration to the extension's implementation function.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct ExtensionTag {
    /// The tag name (method name, e.g., "parse", "install", "override").
    pub tag_name: String,

    /// Keyword arguments passed to the tag.
    /// Values are stored as JSON strings for flexibility.
    pub kwargs: Vec<(String, TagValue)>,
}

impl ExtensionTag {
    /// Create a new extension tag.
    pub fn new(tag_name: String) -> Self {
        Self {
            tag_name,
            kwargs: Vec::new(),
        }
    }

    /// Add a keyword argument.
    pub fn with_kwarg(mut self, key: String, value: TagValue) -> Self {
        self.kwargs.push((key, value));
        self
    }
}

/// A value passed to an extension tag.
///
/// Tags can accept various types of values. We store them in a typed enum
/// for proper handling during extension execution.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub enum TagValue {
    /// A string value.
    String(String),
    /// An integer value.
    Int(i64),
    /// A boolean value.
    Bool(bool),
    /// A list of values.
    List(Vec<TagValue>),
    /// A dictionary of values.
    Dict(Vec<(String, TagValue)>),
    /// A label reference (e.g., "//:requirements.txt").
    Label(String),
    /// None/null value.
    None,
}

impl TagValue {
    /// Convert to a string representation.
    pub fn to_string_value(&self) -> Option<&str> {
        match self {
            TagValue::String(s) => Some(s),
            TagValue::Label(s) => Some(s),
            _ => None,
        }
    }

    /// Convert to a boolean.
    pub fn to_bool(&self) -> Option<bool> {
        match self {
            TagValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Convert to an integer.
    pub fn to_int(&self) -> Option<i64> {
        match self {
            TagValue::Int(i) => Some(*i),
            _ => None,
        }
    }
}

/// A `use_repo()` call to import repositories from an extension.
///
/// # Example
///
/// ```starlark
/// use_repo(pip, "pip", other_name = "pip_internal")
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct UseRepo {
    /// Repository names to import (positional arguments after the extension).
    /// Each string is a repo name that will be made available as `@repo_name`.
    pub repos: Vec<String>,

    /// Repository name remapping (keyword arguments).
    /// Maps apparent name -> actual generated repo name.
    pub repo_mapping: Vec<(String, String)>,
}

impl UseRepo {
    /// Create a new use_repo with no mappings.
    pub fn new() -> Self {
        Self {
            repos: Vec::new(),
            repo_mapping: Vec::new(),
        }
    }

    /// Add a repository to import.
    pub fn add_repo(mut self, repo: String) -> Self {
        self.repos.push(repo);
        self
    }

    /// Add a repository with a name mapping.
    pub fn add_mapping(mut self, apparent_name: String, actual_name: String) -> Self {
        self.repo_mapping.push((apparent_name, actual_name));
        self
    }
}

impl Default for UseRepo {
    fn default() -> Self {
        Self::new()
    }
}
