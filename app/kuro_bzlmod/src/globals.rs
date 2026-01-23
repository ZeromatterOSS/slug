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

use starlark::any::ProvidesStaticType;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::list::UnpackList;
use starlark::values::none::NoneType;
use starlark::values::tuple::UnpackTuple;
use starlark::values::Value;

use crate::types::ArchiveOverride;
use crate::types::BazelDep;
use crate::types::GitOverride;
use crate::types::LocalPathOverride;
use crate::types::MultipleVersionOverride;
use crate::types::Override;
use crate::types::SingleVersionOverride;
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

    /// All use_extension() declarations (for Phase 5).
    pub extensions: Vec<ExtensionUsage>,
}

/// The module() declaration.
#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub name: String,
    pub version: Version,
    pub compatibility_level: u32,
}

/// A use_extension() declaration (placeholder for Phase 5).
#[derive(Debug, Clone)]
pub struct ExtensionUsage {
    pub extension_bzl_file: String,
    pub extension_name: String,
    pub dev_dependency: bool,
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
        #[starlark(require = named, default = "")] repo_name: &str,
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

        let dep = BazelDep {
            name: name.to_owned(),
            version: parsed_version,
            repo_name: if repo_name.is_empty() {
                None
            } else {
                Some(repo_name.to_owned())
            },
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
    /// This is a placeholder for Phase 5 - module extensions.
    /// Currently just records the usage for later processing.
    ///
    /// # Example
    ///
    /// ```starlark
    /// pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
    /// pip.parse(...)
    /// use_repo(pip, "pip_deps")
    /// ```
    fn use_extension<'v>(
        #[starlark(require = pos)] extension_bzl_file: &str,
        #[starlark(require = pos)] extension_name: &str,
        #[starlark(require = named, default = false)] dev_dependency: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let ctx = get_module_context(eval)?;
        let mut ctx = ctx.borrow_mut();

        ctx.extensions.push(ExtensionUsage {
            extension_bzl_file: extension_bzl_file.to_owned(),
            extension_name: extension_name.to_owned(),
            dev_dependency,
        });

        // For now, return None. In Phase 5, this would return an extension proxy object.
        Ok(Value::new_none())
    }

    /// Imports repositories from a module extension.
    ///
    /// This is a placeholder for Phase 5 - module extensions.
    /// Currently a no-op.
    ///
    /// # Example
    ///
    /// ```starlark
    /// use_repo(pip, "pip_deps")
    /// ```
    fn use_repo<'v>(
        #[starlark(require = pos)] extension: Value<'v>,
        #[starlark(args)] repos: UnpackTuple<&str>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // Placeholder for Phase 5
        let _ = extension;
        let _ = repos;
        let _ = eval;
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
        // Currently a no-op - toolchain registration is handled by the build system
        let _ = toolchains;
        let _ = dev_dependency;
        let _ = eval;
        Ok(NoneType)
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
        // Currently a no-op - platform registration is handled by the build system
        let _ = platforms;
        let _ = dev_dependency;
        let _ = eval;
        Ok(NoneType)
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
