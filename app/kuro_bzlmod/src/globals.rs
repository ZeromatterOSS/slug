/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Starlark globals for MODULE.bazel files.
//!
//! This module provides the native functions available in MODULE.bazel files:
//! - `module()` - declares the module's identity
//! - `bazel_dep()` - declares a dependency on another module
//! - `local_path_override()` - overrides a module with a local path
//! - `single_version_override()` - forces a specific version
//! - `multiple_version_override()` - allows multiple versions
//! - `archive_override()` - fetches from an archive URL
//! - `git_override()` - fetches from a git repository
//! - `use_extension()` - uses a module extension (Phase 5)
//! - `use_repo()` - imports repositories from an extension (Phase 5)

use std::cell::RefCell;
use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::any::ProvidesStaticType;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::DictRef;
use starlark::values::list::ListRef;
use starlark::values::list::UnpackList;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use starlark::values::tuple::UnpackTuple;

use crate::types::ArchiveOverride;
use crate::types::BazelDep;
use crate::types::ExtensionTag;
use crate::types::ExtensionUsage;
use crate::types::GitOverride;
use crate::types::LocalPathOverride;
use crate::types::MultipleVersionOverride;
use crate::types::Override;
use crate::types::RegisteredItem;
use crate::types::SingleVersionOverride;
use crate::types::TagValue;
use crate::types::UseRepo;
use crate::version::Version;

/// Context for MODULE.bazel evaluation.
///
/// This context accumulates the parsed directives during evaluation.
#[derive(Debug, Default, ProvidesStaticType)]
pub struct ModuleFileContext {
    /// The module declaration, if present.
    pub module: Option<ModuleDecl>,

    /// All bazel_dep() declarations.
    pub bazel_deps: Vec<BazelDep>,

    /// All override declarations.
    pub overrides: Vec<Override>,

    /// All use_extension() declarations with their tags and imports.
    pub extensions: Vec<ExtensionUsage>,

    /// Counter for extension IDs (used to link use_repo to use_extension).
    #[allow(dead_code)]
    extension_counter: usize,

    /// Repository rule invocations from use_repo_rule() calls in MODULE.bazel.
    /// e.g., http_file(name = "toml2json_linux_amd64", ...)
    pub repo_rule_invocations: Vec<RepoRuleInvocation>,

    /// Toolchain labels from register_toolchains() calls, with dev_dependency tracking.
    pub registered_toolchains: Vec<RegisteredItem>,

    /// Execution platform labels from register_execution_platforms() calls, with dev_dependency tracking.
    pub registered_execution_platforms: Vec<RegisteredItem>,
}

/// A repository rule invocation from MODULE.bazel (via use_repo_rule).
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct RepoRuleInvocation {
    /// The repo name (from `name` attribute).
    pub name: String,
    /// The rule source: "bzl_path%rule_name".
    pub rule_source: String,
    /// Attribute values (excluding name).
    pub attrs: fxhash::FxHashMap<String, TagValue>,
}

/// The module() declaration.
#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub name: String,
    pub version: Version,
    pub compatibility_level: u32,
}

// ============================================================================
// ExtensionProxy - Starlark value for capturing extension tag calls
// ============================================================================

/// A proxy object returned by `use_extension()` that captures tag method calls.
///
/// When you call a method on this object (e.g., `pip.parse(...)`), it records
/// the call as an ExtensionTag that will be processed by the extension.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ExtensionProxy {
    /// Index of the extension in ModuleFileContext.extensions.
    extension_index: usize,
    /// Extension bzl file for debugging.
    extension_bzl_file: String,
    /// Extension name for debugging.
    extension_name: String,
}

impl Display for ExtensionProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "<extension_proxy {}%{}>",
            self.extension_bzl_file, self.extension_name
        )
    }
}

starlark_simple_value!(ExtensionProxy);

#[starlark_value(type = "extension_proxy")]
impl<'v> StarlarkValue<'v> for ExtensionProxy {
    fn has_attr(&self, _attribute: &str, _heap: Heap<'v>) -> bool {
        // Extension proxies accept any attribute (tag class name)
        true
    }

    fn dir_attr(&self) -> Vec<String> {
        // We don't know what tag classes exist, so return empty
        Vec::new()
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        // Return a tag invoker for this attribute (tag class)
        let invoker = ExtensionTagInvoker {
            extension_index: self.extension_index,
            tag_name: attribute.to_string(),
        };
        Some(heap.alloc(invoker))
    }
}

impl ExtensionProxy {
    /// Get the extension index.
    pub fn index(&self) -> usize {
        self.extension_index
    }
}

/// A callable that records a tag invocation when called.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ExtensionTagInvoker {
    /// Index of the extension in ModuleFileContext.extensions.
    extension_index: usize,
    /// The tag name (method name being invoked).
    tag_name: String,
}

impl Display for ExtensionTagInvoker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<tag_invoker {}>", self.tag_name)
    }
}

starlark_simple_value!(ExtensionTagInvoker);

#[starlark_value(type = "extension_tag_invoker")]
impl<'v> StarlarkValue<'v> for ExtensionTagInvoker {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        // Create the tag with all keyword arguments
        let mut tag = ExtensionTag::new(self.tag_name.clone());

        // Convert kwargs to TagValue using names_map()
        let kwargs = args.names_map()?;
        for (name, value) in kwargs.iter() {
            let tag_value = starlark_to_tag_value(*value)?;
            tag.kwargs.push((name.as_str().to_string(), tag_value));
        }

        // Add tag to the appropriate extension
        if let Some(ext) = ctx.extensions.get_mut(self.extension_index) {
            ext.tags.push(tag);
        }

        Ok(Value::new_none())
    }
}

/// Convert a Starlark value to a TagValue.
fn starlark_to_tag_value(value: Value) -> starlark::Result<TagValue> {
    if value.is_none() {
        return Ok(TagValue::None);
    }

    if let Some(s) = value.unpack_str() {
        // Check if it looks like a label
        if s.starts_with("//") || s.starts_with("@") || s.starts_with(":") {
            return Ok(TagValue::Label(s.to_string()));
        }
        return Ok(TagValue::String(s.to_string()));
    }

    if let Some(b) = value.unpack_bool() {
        return Ok(TagValue::Bool(b));
    }

    if let Some(i) = value.unpack_i32() {
        return Ok(TagValue::Int(i as i64));
    }

    if let Some(list) = ListRef::from_value(value) {
        let items: Vec<TagValue> = list
            .iter()
            .map(starlark_to_tag_value)
            .collect::<starlark::Result<Vec<_>>>()?;
        return Ok(TagValue::List(items));
    }

    if let Some(dict) = DictRef::from_value(value) {
        let items: Vec<(String, TagValue)> = dict
            .iter()
            .map(|(k, v)| {
                let key = k
                    .unpack_str()
                    .ok_or_else(|| {
                        starlark::Error::new_other(anyhow::anyhow!(
                            "Dict keys must be strings in extension tags"
                        ))
                    })?
                    .to_string();
                let value = starlark_to_tag_value(v)?;
                Ok((key, value))
            })
            .collect::<starlark::Result<Vec<_>>>()?;
        return Ok(TagValue::Dict(items));
    }

    // For complex types, convert to string representation
    Ok(TagValue::String(value.to_repr()))
}

/// Register all MODULE.bazel globals.
pub fn register_module_file_globals(globals: &mut GlobalsBuilder) {
    register_module_globals(globals);
}

#[starlark_module]
fn register_module_globals(globals: &mut GlobalsBuilder) {
    /// Declares the identity of the Bazel module.
    ///
    /// This should be called at most once in a MODULE.bazel file.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the module. Required for modules that will be
    ///   published to a registry.
    /// * `version` - The version of the module (relaxed SemVer format).
    /// * `compatibility_level` - The compatibility level. Modules with different
    ///   compatibility levels cannot coexist unless explicitly allowed.
    /// * `repo_name` - The repository name for the module (defaults to `name`).
    /// * `bazel_compatibility` - List of Bazel version constraints (currently ignored).
    ///
    /// # Example
    ///
    /// ```starlark
    /// module(
    ///     name = "my_project",
    ///     version = "1.0.0",
    ///     compatibility_level = 1,
    /// )
    /// ```
    fn module<'v>(
        #[starlark(require = named, default = "")] name: &str,
        #[starlark(require = named, default = "")] version: &str,
        #[starlark(require = named, default = 0)] compatibility_level: i32,
        #[starlark(require = named, default = "")] repo_name: &str,
        #[starlark(require = named, default = UnpackList::default())]
        bazel_compatibility: UnpackList<&str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        if ctx.module.is_some() {
            return Err(starlark::Error::new_other(anyhow::anyhow!(
                "module() can only be called once per MODULE.bazel file"
            )));
        }

        let parsed_version = if version.is_empty() {
            Version::empty()
        } else {
            Version::parse(version)
                .map_err(|e| starlark::Error::new_other(anyhow::anyhow!("{}", e)))?
        };

        ctx.module = Some(ModuleDecl {
            name: name.to_owned(),
            version: parsed_version,
            compatibility_level: compatibility_level as u32,
        });

        // repo_name and bazel_compatibility are currently ignored but accepted
        let _ = repo_name;
        let _ = bazel_compatibility;

        Ok(NoneType)
    }

    /// Declares a dependency on another Bazel module.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the module to depend on.
    /// * `version` - The minimum required version.
    /// * `max_compatibility_level` - Maximum allowed compatibility level (currently ignored).
    /// * `repo_name` - Override the repository name for this dependency.
    /// * `dev_dependency` - If true, this dependency is only needed for development.
    ///
    /// # Example
    ///
    /// ```starlark
    /// bazel_dep(name = "rules_cc", version = "0.0.9")
    /// bazel_dep(name = "rules_rust", version = "0.40.0", dev_dependency = True)
    /// ```
    fn bazel_dep<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = "")] version: &str,
        #[starlark(require = named, default = -1)] max_compatibility_level: i32,
        #[starlark(require = named, default = NoneOr::None)] repo_name: NoneOr<&str>,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        let parsed_version = if version.is_empty() {
            Version::empty()
        } else {
            Version::parse(version)
                .map_err(|e| starlark::Error::new_other(anyhow::anyhow!("{}", e)))?
        };

        let repo_name_str = match repo_name {
            NoneOr::None => None,
            NoneOr::Other(s) if s.is_empty() => None,
            NoneOr::Other(s) => Some(s.to_owned()),
        };

        let dep = BazelDep {
            name: name.to_owned(),
            version: parsed_version,
            repo_name: repo_name_str,
            dev_dependency,
        };

        ctx.bazel_deps.push(dep);

        // max_compatibility_level is currently ignored
        let _ = max_compatibility_level;

        Ok(NoneType)
    }

    /// Overrides a module to use a local filesystem path.
    ///
    /// # Arguments
    ///
    /// * `module_name` - The module to override.
    /// * `path` - The local path (relative to workspace root).
    ///
    /// # Example
    ///
    /// ```starlark
    /// local_path_override(
    ///     module_name = "my_local_module",
    ///     path = "../my-local-module",
    /// )
    /// ```
    fn local_path_override<'v>(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named)] path: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        ctx.overrides.push(Override::LocalPath(LocalPathOverride {
            module_name: module_name.to_owned(),
            path: path.to_owned(),
        }));

        Ok(NoneType)
    }

    /// Forces a specific version of a module.
    ///
    /// # Arguments
    ///
    /// * `module_name` - The module to override.
    /// * `version` - The exact version to use.
    /// * `registry` - Optional alternative registry URL.
    /// * `patches` - Patches to apply.
    /// * `patch_strip` - Number of leading path components to strip from patches.
    ///
    /// # Example
    ///
    /// ```starlark
    /// single_version_override(
    ///     module_name = "protobuf",
    ///     version = "3.19.2",
    /// )
    /// ```
    fn single_version_override<'v>(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named, default = "")] version: &str,
        #[starlark(require = named, default = "")] registry: &str,
        #[starlark(require = named, default = UnpackList::default())] patches: UnpackList<&str>,
        #[starlark(require = named, default = 0)] patch_strip: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        let parsed_version = if version.is_empty() {
            Version::empty()
        } else {
            Version::parse(version)
                .map_err(|e| starlark::Error::new_other(anyhow::anyhow!("{}", e)))?
        };

        ctx.overrides
            .push(Override::SingleVersion(SingleVersionOverride {
                module_name: module_name.to_owned(),
                version: parsed_version,
                registry: if registry.is_empty() {
                    None
                } else {
                    Some(registry.to_owned())
                },
                patches: patches.items.iter().map(|s| s.to_string()).collect(),
                patch_strip: patch_strip as u32,
            }));

        Ok(NoneType)
    }

    /// Allows multiple versions of a module to coexist.
    ///
    /// # Arguments
    ///
    /// * `module_name` - The module to override.
    /// * `versions` - The allowed versions.
    /// * `registry` - Optional alternative registry URL.
    ///
    /// # Example
    ///
    /// ```starlark
    /// multiple_version_override(
    ///     module_name = "protobuf",
    ///     versions = ["3.18.0", "3.19.0"],
    /// )
    /// ```
    fn multiple_version_override<'v>(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named)] versions: UnpackList<&str>,
        #[starlark(require = named, default = "")] registry: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        let parsed_versions: Vec<Version> = versions
            .items
            .iter()
            .map(|v| Version::parse(v))
            .collect::<kuro_error::Result<Vec<_>>>()
            .map_err(|e| starlark::Error::new_other(anyhow::anyhow!("{}", e)))?;

        ctx.overrides
            .push(Override::MultipleVersion(MultipleVersionOverride {
                module_name: module_name.to_owned(),
                versions: parsed_versions,
                registry: if registry.is_empty() {
                    None
                } else {
                    Some(registry.to_owned())
                },
            }));

        Ok(NoneType)
    }

    /// Overrides a module to fetch from an archive URL.
    ///
    /// # Arguments
    ///
    /// * `module_name` - The module to override.
    /// * `urls` - URLs to download the archive from.
    /// * `integrity` - SRI hash for verification.
    /// * `strip_prefix` - Directory prefix to strip from archive.
    /// * `patches` - Patches to apply.
    /// * `patch_strip` - Number of leading path components to strip from patches.
    ///
    /// # Example
    ///
    /// ```starlark
    /// archive_override(
    ///     module_name = "rules_cc",
    ///     urls = ["https://example.com/rules_cc.tar.gz"],
    ///     integrity = "sha256-...",
    /// )
    /// ```
    fn archive_override<'v>(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named)] urls: UnpackList<&str>,
        #[starlark(require = named, default = "")] integrity: &str,
        #[starlark(require = named, default = "")] strip_prefix: &str,
        #[starlark(require = named, default = UnpackList::default())] patches: UnpackList<&str>,
        #[starlark(require = named, default = 0)] patch_strip: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        ctx.overrides.push(Override::Archive(ArchiveOverride {
            module_name: module_name.to_owned(),
            urls: urls.items.iter().map(|s| s.to_string()).collect(),
            integrity: if integrity.is_empty() {
                None
            } else {
                Some(integrity.to_owned())
            },
            strip_prefix: if strip_prefix.is_empty() {
                None
            } else {
                Some(strip_prefix.to_owned())
            },
            patches: patches.items.iter().map(|s| s.to_string()).collect(),
            patch_strip: patch_strip as u32,
        }));

        Ok(NoneType)
    }

    /// Overrides a module to fetch from a git repository.
    ///
    /// # Arguments
    ///
    /// * `module_name` - The module to override.
    /// * `remote` - Git remote URL.
    /// * `commit` - Commit hash to checkout.
    /// * `shallow_since` - Date for shallow clone optimization.
    /// * `patches` - Patches to apply.
    /// * `patch_strip` - Number of leading path components to strip from patches.
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
    fn git_override<'v>(
        #[starlark(require = named)] module_name: &str,
        #[starlark(require = named)] remote: &str,
        #[starlark(require = named, default = "")] commit: &str,
        #[starlark(require = named, default = "")] shallow_since: &str,
        #[starlark(require = named, default = UnpackList::default())] patches: UnpackList<&str>,
        #[starlark(require = named, default = 0)] patch_strip: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        ctx.overrides.push(Override::Git(GitOverride {
            module_name: module_name.to_owned(),
            remote: remote.to_owned(),
            commit: commit.to_owned(),
            shallow_since: if shallow_since.is_empty() {
                None
            } else {
                Some(shallow_since.to_owned())
            },
            patches: patches.items.iter().map(|s| s.to_string()).collect(),
            patch_strip: patch_strip as u32,
        }));

        Ok(NoneType)
    }

    /// Uses a module extension.
    ///
    /// Returns an extension proxy object that captures tag method calls.
    /// Tags are recorded and processed by the extension's implementation.
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
    fn use_extension<'v>(
        #[starlark(require = pos)] extension_bzl_file: &str,
        #[starlark(require = pos)] extension_name: &str,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        #[starlark(require = named, default = false)] isolate: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        // Create the extension usage record
        let mut ext = ExtensionUsage::new(extension_bzl_file.to_owned(), extension_name.to_owned());
        ext.dev_dependency = dev_dependency;
        ext.isolate = isolate;

        // Get the index for this extension
        let extension_index = ctx.extensions.len();
        ctx.extensions.push(ext);

        // Create and return the proxy
        let proxy = ExtensionProxy {
            extension_index,
            extension_bzl_file: extension_bzl_file.to_owned(),
            extension_name: extension_name.to_owned(),
        };

        Ok(eval.heap().alloc(proxy))
    }

    /// Imports repositories from a module extension.
    ///
    /// Positional string arguments are repo names to import directly.
    /// Keyword arguments map apparent names to actual repo names.
    ///
    /// # Example
    ///
    /// ```starlark
    /// use_repo(pip, "pip", "pip_internal")  # Import repos as-is
    /// use_repo(maven, maven_deps = "maven")  # Import "maven" as "@maven_deps"
    /// ```
    fn use_repo<'v>(
        #[starlark(require = pos)] extension: Value<'v>,
        #[starlark(args)] repos: UnpackTuple<&str>,
        #[starlark(kwargs)] kwargs: starlark::collections::SmallMap<String, Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        // Get the extension proxy
        let proxy = extension.downcast_ref::<ExtensionProxy>().ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "use_repo() first argument must be an extension from use_extension()"
            ))
        })?;

        // Create UseRepo record
        let mut use_repo = UseRepo::new();

        // Add positional repo names
        for repo in repos.items {
            use_repo.repos.push(repo.to_string());
        }

        // Add keyword repo-mapping: apparent_name = "actual_name"
        for (apparent_name, actual_value) in kwargs.iter() {
            if let Some(actual_name) = actual_value.unpack_str() {
                use_repo
                    .repo_mapping
                    .push((apparent_name.clone(), actual_name.to_owned()));
            }
        }

        // Add to the appropriate extension
        if let Some(ext) = ctx.extensions.get_mut(proxy.index()) {
            ext.imports.push(use_repo);
        }

        Ok(NoneType)
    }

    /// Registers toolchains for use in the module.
    ///
    /// This is recorded but toolchain resolution is handled separately.
    ///
    /// # Example
    ///
    /// ```starlark
    /// register_toolchains("@rules_cc//cc:all")
    /// ```
    fn register_toolchains<'v>(
        #[starlark(args)] toolchains: UnpackTuple<&str>,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();
        for tc in toolchains.items {
            ctx.registered_toolchains.push(RegisteredItem {
                label: tc.to_owned(),
                dev_dependency,
            });
        }
        Ok(NoneType)
    }

    /// Returns a repo rule callable for use in MODULE.bazel.
    ///
    /// This is a Bazel 7.1+ feature that allows creating repository rules
    /// inline in MODULE.bazel without using extensions.
    ///
    /// # Example
    ///
    /// ```starlark
    /// local_runtime_repo = use_repo_rule("//python:local_runtime_repo.bzl", "local_runtime_repo")
    /// local_runtime_repo(name = "system_python", ...)
    /// ```
    fn use_repo_rule<'v>(
        #[starlark(require = pos)] rule_bzl_file: &str,
        #[starlark(require = pos)] rule_name: &str,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _ = dev_dependency;

        let proxy = RepoRuleProxy {
            rule_bzl_file: rule_bzl_file.to_owned(),
            rule_name: rule_name.to_owned(),
        };

        Ok(eval.heap().alloc(proxy))
    }

    /// Registers execution platforms for use in the module.
    ///
    /// This is recorded but platform resolution is handled separately.
    ///
    /// # Example
    ///
    /// ```starlark
    /// register_execution_platforms("@local_config_platform//:host")
    /// ```
    fn register_execution_platforms<'v>(
        #[starlark(args)] platforms: UnpackTuple<&str>,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();
        for p in platforms.items {
            ctx.registered_execution_platforms.push(RegisteredItem {
                label: p.to_owned(),
                dev_dependency,
            });
        }
        Ok(NoneType)
    }

    /// Overrides repositories generated by a module extension.
    ///
    /// For each `repo_name=dep_name` in the keyword arguments, when the
    /// extension creates a repo called `repo_name`, it will be replaced
    /// by the repo provided by `bazel_dep(name=dep_name)`.
    ///
    /// # Example
    ///
    /// ```starlark
    /// ext = use_extension("@rules_rs//rs/experimental:rules_rust.bzl", "rules_rust")
    /// override_repo(ext, rules_rust = "rules_rust")
    /// ```
    fn override_repo<'v>(
        #[starlark(require = pos)] extension_proxy: Value<'v>,
        #[starlark(kwargs)] kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let proxy = extension_proxy
            .downcast_ref::<ExtensionProxy>()
            .ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "override_repo() first argument must be a module extension proxy from use_extension()"
                ))
            })?;

        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        // Record the overrides on the extension
        if let Some(ext) = ctx.extensions.get_mut(proxy.index()) {
            if let Some(dict) = DictRef::from_value(kwargs) {
                for (key, value) in dict.iter() {
                    if let (Some(repo_name), Some(dep_name)) =
                        (key.unpack_str(), value.unpack_str())
                    {
                        ext.repo_overrides
                            .push((repo_name.to_owned(), dep_name.to_owned()));
                    }
                }
            }
        }

        Ok(NoneType)
    }

    /// Injects repositories into a module extension's visibility.
    ///
    /// Makes additional repos (from `bazel_dep`) visible to an extension
    /// that doesn't normally have access to them.
    ///
    /// # Example
    ///
    /// ```starlark
    /// crate = use_extension("@rules_rs//rs:extensions.bzl", "crate")
    /// inject_repo(crate, "zlib")
    /// ```
    fn inject_repo<'v>(
        #[starlark(require = pos)] extension_proxy: Value<'v>,
        #[starlark(args)] repos: UnpackTuple<&str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let proxy = extension_proxy
            .downcast_ref::<ExtensionProxy>()
            .ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "inject_repo() first argument must be a module extension proxy from use_extension()"
                ))
            })?;

        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        // Record the injected repos on the extension
        if let Some(ext) = ctx.extensions.get_mut(proxy.index()) {
            for repo in repos.items {
                ext.injected_repos.push(repo.to_owned());
            }
        }

        Ok(NoneType)
    }
}

// ============================================================================
// RepoRuleProxy - Starlark value for use_repo_rule() return value
// ============================================================================

/// A proxy object returned by `use_repo_rule()` that accepts calls to create repos.
///
/// When called (e.g., `local_runtime_repo(name = "system_python", ...)`), it records
/// the invocation as a no-op since Kuro uses synthetic repos for these.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct RepoRuleProxy {
    /// The bzl file containing the repo rule.
    rule_bzl_file: String,
    /// The repo rule name.
    rule_name: String,
}

impl Display for RepoRuleProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "<repo_rule_proxy {}%{}>",
            self.rule_bzl_file, self.rule_name
        )
    }
}

starlark_simple_value!(RepoRuleProxy);

#[starlark_value(type = "repo_rule_proxy")]
impl<'v> StarlarkValue<'v> for RepoRuleProxy {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Record the repo rule invocation so it can be materialized later.
        // This is called when e.g. http_file(name="toml2json_linux_amd64", ...)
        // appears in MODULE.bazel via use_repo_rule().
        let kwargs = args.names_map()?;
        let name = kwargs
            .get("name")
            .and_then(|v| v.unpack_str())
            .unwrap_or("");

        if !name.is_empty() {
            // Record in the module context so it can be processed as a cell
            if let Ok(module_ctx) = get_module_context(eval) {
                let mut ctx = module_ctx.borrow_mut();
                // Store as a repo rule invocation with rule source info
                let rule_source = format!("{}%{}", self.rule_bzl_file, self.rule_name);
                let mut attrs = fxhash::FxHashMap::default();
                for (key, value) in kwargs.iter() {
                    let key_str = key.as_str();
                    if key_str != "name" {
                        attrs.insert(key_str.to_owned(), starlark_to_tag_value(*value)?);
                    }
                }
                ctx.repo_rule_invocations.push(RepoRuleInvocation {
                    name: name.to_owned(),
                    rule_source,
                    attrs,
                });
            }
        }

        Ok(Value::new_none())
    }
}

/// Get the module context from the evaluator.
fn get_module_context<'v, 'a>(
    eval: &Evaluator<'v, 'a, '_>,
) -> starlark::Result<&'a RefCell<ModuleFileContext>> {
    eval.extra
        .and_then(|e| e.downcast_ref::<RefCell<ModuleFileContext>>())
        .ok_or_else(|| {
            starlark::Error::new_other(anyhow::anyhow!(
                "MODULE.bazel context not found in evaluator"
            ))
        })
}

/// Creates a new ModuleFileContext.
pub fn new_module_file_context() -> RefCell<ModuleFileContext> {
    RefCell::new(ModuleFileContext::default())
}
