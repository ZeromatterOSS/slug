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
//! Plan Reference: `thoughts/shared/plans/slug-bazel-subplans/02-bzlmod.md` Phase 5
//!
//! ## Current Status: FULLY IMPLEMENTED
//!
//! This provides the `module_ctx` object passed to module extension implementations.
//! The `modules` property returns real module data with tags populated from
//! slug_bzlmod's aggregated extension data.
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

mod context;
mod metadata;
mod methods;
mod module;
mod os;
mod tags;

#[cfg(test)]
mod tests;

use starlark::environment::GlobalsBuilder;
use starlark::starlark_module;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;

pub use crate::module_ctx::context::ModuleContext;
pub use crate::module_ctx::context::SerializedModule;
pub use crate::module_ctx::metadata::FactsValue;
pub use crate::module_ctx::metadata::StarlarkModuleExtensionMetadata;
pub use crate::module_ctx::metadata::empty_facts;
pub use crate::module_ctx::metadata::validate_facts_value;
pub use crate::module_ctx::module::BazelModule;
pub use crate::module_ctx::os::RepositoryOs;
pub use crate::module_ctx::tags::BazelModuleTags;
pub use crate::module_ctx::tags::SerializedTag;
pub use crate::module_ctx::tags::SerializedTagValue;
pub use crate::module_ctx::tags::TagInstance;
pub use crate::module_ctx::tags::coerced_attr_to_serialized_tag_value;
pub use crate::module_ctx::tags::default_for_attr_type;

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
