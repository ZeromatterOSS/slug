/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use starlark::collections::SmallMap;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::StringValue;
use starlark::values::Value;
use starlark::values::ValueOfUnchecked;
use starlark::values::dict::AllocDict;
use starlark::values::list::AllocList;
use starlark::values::list::UnpackList;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;

use crate::interpreter::build_context::BuildContext;
use crate::interpreter::build_context::PerFileTypeContext;
use crate::interpreter::globspec::GlobSpec;
use crate::interpreter::module_internals::ModuleInternals;

/// Register Bazel-specific module-level globals.
///
/// These are functions that can be called at the top level of .bzl files.
#[starlark_module]
pub(crate) fn register_bzl_module_globals(globals: &mut GlobalsBuilder) {
    /// Creates a Label object from a string.
    ///
    /// This is Bazel's built-in `Label()` function that creates a label from a string.
    /// Labels can be absolute (starting with `//` or `@`) or relative to the current package.
    ///
    /// Examples:
    /// ```python
    /// Label("//foo:bar")  # Absolute label in the main repository
    /// Label("@repo//foo:bar")  # Label in an external repository
    /// Label(":target")  # Relative to current package (only in BUILD files)
    /// ```
    ///
    /// See: https://bazel.build/rules/lib/builtins/Label
    fn Label<'v>(
        #[starlark(require = pos)] label_string: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StringValue<'v>> {
        // In Bazel, Label() resolves relative to the file where it appears:
        // - In a .bzl file: // resolves to the .bzl file's repository
        // - In a BUILD file: // resolves to the BUILD file's repository
        // This is critical for rules packages (e.g., rules_cc) that use
        // Label("//:target") to refer to their own repo's targets.
        let build_ctx = BuildContext::from_context(eval)?;

        // Use starlark_path().cell() which returns:
        // - For .bzl files: the .bzl file's cell (e.g., "rules_cc")
        // - For BUILD files: the BUILD file's cell (e.g., "manual_test")
        let file_cell = build_ctx.starlark_path().cell();

        let resolved = if label_string.starts_with('@') {
            // Already fully qualified with repository
            label_string.to_owned()
        } else if label_string.starts_with("//") {
            // Absolute path within the current file's repository
            format!("@{}{}", file_cell, label_string)
        } else {
            // Relative label (:target or bare target)
            // Get the package path based on the current file type
            let pkg_path = match &build_ctx.additional {
                PerFileTypeContext::Build(module) => module
                    .buildfile_path()
                    .package()
                    .to_cell_path()
                    .path()
                    .as_str()
                    .to_owned(),
                PerFileTypeContext::Bzl(bzl_ctx) => {
                    // For .bzl files, the "package" is the directory containing the .bzl file
                    bzl_ctx.bzl_path.path_parent().path().as_str().to_owned()
                }
                _ => {
                    // For other file types, try base_path as fallback
                    build_ctx.base_path()?.path().as_str().to_owned()
                }
            };
            let target = label_string.strip_prefix(':').unwrap_or(label_string);
            format!("@{}//{}:{}", file_cell, pkg_path, target)
        };

        // TODO(label): Return actual StarlarkTargetLabel once label resolution is wired up
        Ok(eval.heap().alloc_str(&resolved))
    }

    /// Declares the visibility of the current .bzl file.
    ///
    /// This function is called at the module level in Bazel .bzl files to control
    /// which packages can load the file. For example:
    ///
    /// ```python
    /// visibility("public")  # Any package can load this file
    /// visibility("private")  # Only the same package can load this file
    /// visibility(["//some/package:__subpackages__"])  # Specific packages
    /// ```
    ///
    /// In Kuro, this is currently a no-op stub that accepts the argument but
    /// does not enforce visibility. This allows loading of bazel_tools and other
    /// BCR modules that use this function.
    ///
    /// See: https://bazel.build/rules/lib/globals/bzl#visibility
    fn visibility<'v>(#[starlark(require = pos)] _value: Value<'v>) -> starlark::Result<NoneType> {
        // TODO(bzlmod): Implement .bzl file visibility enforcement.
        // Currently a no-op - Kuro doesn't enforce .bzl file visibility yet.
        // The value can be:
        // - "public" - visible to all packages
        // - "private" - visible only within the same package
        // - A list of package specifications like ["//foo:__pkg__", "//bar:__subpackages__"]
        Ok(NoneType)
    }
}

#[starlark_module]
pub(crate) fn register_module_natives(globals: &mut GlobalsBuilder) {
    /// Declares the license type for the targets in this BUILD file.
    ///
    /// This is a legacy Bazel built-in function for declaring licenses. In modern
    /// Bazel (9.0+), licenses are deprecated and this is a no-op.
    ///
    /// Example:
    /// ```python
    /// licenses(["notice"])  # Apache 2.0
    /// licenses(["restricted", "notice"])  # Multiple license types
    /// ```
    ///
    /// See: https://bazel.build/reference/be/functions#licenses
    fn licenses<'v>(
        #[starlark(require = pos)] _license_types: UnpackListOrTuple<String>,
    ) -> starlark::Result<NoneType> {
        // This is a legacy/deprecated function in Bazel 9.0+
        // Currently a no-op - Kuro doesn't track license metadata.
        Ok(NoneType)
    }

    /// Declares which files in a package are publicly visible.
    ///
    /// This is a Bazel built-in function that marks files for export. In Kuro,
    /// this is currently a no-op stub - all files in a package are accessible.
    ///
    /// Example:
    /// ```python
    /// exports_files(["version.bzl", "globals.bzl"])
    /// exports_files(["data.txt"], visibility = ["//some/package:__pkg__"])
    /// ```
    ///
    /// See: https://bazel.build/reference/be/functions#exports_files
    fn exports_files<'v>(
        #[starlark(require = pos)] srcs: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _visibility: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _licenses: UnpackListOrTuple<String>,
    ) -> starlark::Result<NoneType> {
        // TODO(bazel-compat): Implement file visibility enforcement.
        // Currently a no-op - Kuro doesn't enforce file-level visibility.
        let _unused = srcs;
        Ok(NoneType)
    }

    /// Declares a toolchain for use by rules that support toolchain resolution.
    ///
    /// This is a Bazel built-in function for declaring toolchains. In Kuro,
    /// this is currently a no-op stub - toolchain resolution is not yet implemented.
    ///
    /// Example:
    /// ```python
    /// toolchain(
    ///     name = "cc_toolchain",
    ///     toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
    ///     toolchain = ":cc_compiler",
    /// )
    /// ```
    ///
    /// See: https://bazel.build/reference/be/platforms-and-toolchains#toolchain
    fn toolchain<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] toolchain_type: &str,
        #[starlark(require = named)] toolchain: &str,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _exec_compatible_with: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _target_compatible_with: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _target_settings: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _visibility: UnpackListOrTuple<String>,
    ) -> starlark::Result<NoneType> {
        // TODO(toolchains): Implement toolchain registration and resolution.
        // Currently a no-op - Kuro doesn't yet support Bazel-style toolchains.
        let _unused = (name, toolchain_type, toolchain);
        Ok(NoneType)
    }

    // Note: alias() is implemented in native_rules.rs as a proper native rule

    /// Groups a set of files under a single name for convenience.
    ///
    /// This is a Bazel built-in rule that creates a named reference to a set of files.
    /// Other rules can depend on a filegroup instead of listing individual files.
    ///
    /// Example:
    /// ```python
    /// filegroup(
    ///     name = "headers",
    ///     srcs = ["foo.h", "bar.h"],
    /// )
    ///
    /// cc_library(
    ///     name = "lib",
    ///     hdrs = [":headers"],  # Use the filegroup
    /// )
    /// ```
    ///
    /// See: https://bazel.build/reference/be/general#filegroup
    fn filegroup<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(kwargs)] _kwargs: Value<'v>,
    ) -> starlark::Result<NoneType> {
        // TODO(filegroup): Implement filegroup target that forwards its srcs.
        // Currently a no-op stub that allows parsing of BUILD files that use filegroup.
        // A full implementation would register a target that makes srcs available
        // to dependent rules.
        let _unused = name;
        Ok(NoneType)
    }

    /// Legacy cc_toolchain_suite rule (BUILD file version).
    ///
    /// This is a Bazel built-in native rule that was used before toolchain resolution.
    /// In modern Bazel (and rules_cc 0.2.16+), this is deprecated in favor of toolchain()
    /// rules, but native cc_toolchain_suite must still exist for backwards compatibility.
    ///
    /// Currently a no-op stub that allows parsing.
    fn cc_toolchain_suite<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(kwargs)] _kwargs: Value<'v>,
    ) -> starlark::Result<NoneType> {
        // TODO(toolchains): Implement cc_toolchain_suite target registration.
        // Currently a no-op stub for parsing compatibility.
        let _unused = name;
        Ok(NoneType)
    }

    /// Legacy cc_toolchain rule (BUILD file version).
    ///
    /// This is a Bazel built-in native rule for C++ toolchain definition.
    /// In modern Bazel with rules_cc 0.2.16+, the pure Starlark cc_toolchain rule
    /// is preferred, but native cc_toolchain must exist for backwards compatibility.
    ///
    /// Currently a no-op stub that allows parsing.
    fn cc_toolchain<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(kwargs)] _kwargs: Value<'v>,
    ) -> starlark::Result<NoneType> {
        // TODO(toolchains): Implement cc_toolchain target registration.
        // Currently a no-op stub for parsing compatibility.
        let _unused = name;
        Ok(NoneType)
    }

    /// Check if the target with `name` has already been defined,
    /// returns `True` if it has.
    ///
    /// Note that this function checks for the existence of a _target_ rather than a _rule_.
    /// In general use of this function is discouraged, as it makes definitions of rules not compose.
    fn rule_exists(name: &str, eval: &mut Evaluator) -> starlark::Result<bool> {
        Ok(ModuleInternals::from_context(eval, "rule_exists")?.target_exists(name))
    }

    /// Called in a `BUCK` file to declare the oncall contact details for
    /// all the targets defined. Must be called at most once, before any targets
    /// have been declared. Errors if called from a `.bzl` file.
    fn oncall(
        #[starlark(require = pos)] name: &str,
        eval: &mut Evaluator,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "oncall")?;
        internals.set_oncall(name)?;
        Ok(NoneType)
    }

    /// Called in a `BUCK` file to retrieve the previously set `oncall`, or `None` if none has been set.
    /// It is an error to call `oncall` after calling this function.
    fn read_oncall<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneOr<StringValue<'v>>> {
        let internals = ModuleInternals::from_context(eval, "read_oncall")?;
        match internals.get_oncall() {
            None => Ok(NoneOr::None),
            Some(oncall) => Ok(NoneOr::Other(eval.heap().alloc_str(oncall.as_str()))),
        }
    }

    fn implicit_package_symbol<'v>(
        name: &str,
        default: Option<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let internals = ModuleInternals::from_context(eval, "implicit_package_symbol")?;
        match internals.get_package_implicit(name) {
            None => Ok(default.unwrap_or_else(Value::new_none)),
            Some(v) => {
                // FIXME(ndmitchell): Document why this is safe
                Ok(unsafe { v.unchecked_frozen_value().to_value() })
            }
        }
    }
}

/// Bazel-compatible `native` module.
///
/// This module provides access to native rule functions and built-in functions
/// when called from .bzl files. In Bazel, `native.*` functions provide access to
/// built-in functionality that would otherwise only be available in BUILD files.
///
/// Example usage in a .bzl file:
/// ```python
/// def my_macro(name):
///     native.cc_library(
///         name = name,
///         srcs = native.glob(["*.cc"]),
///     )
/// ```
#[starlark_module]
fn bazel_native_module(registry: &mut GlobalsBuilder) {
    /// Legacy native cc_toolchain_suite rule.
    ///
    /// This is a Bazel built-in native rule that was used before toolchain resolution.
    /// In modern Bazel (and rules_cc 0.2.16+), this is deprecated in favor of toolchain()
    /// rules, but the native.cc_toolchain_suite function must still exist for backwards
    /// compatibility with the wrapper in rules_cc.
    ///
    /// Currently a no-op stub that allows parsing.
    fn cc_toolchain_suite<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = NoneOr::None)] _toolchains: NoneOr<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _visibility: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _tags: UnpackListOrTuple<String>,
        #[starlark(kwargs)] _kwargs: Value<'v>,
    ) -> starlark::Result<NoneType> {
        // TODO(toolchains): Implement cc_toolchain_suite target registration.
        // Currently a no-op stub for parsing compatibility.
        let _unused = name;
        Ok(NoneType)
    }

    /// Legacy native cc_toolchain rule.
    ///
    /// This is a Bazel built-in native rule for C++ toolchain definition.
    /// In modern Bazel with rules_cc 0.2.16+, the pure Starlark cc_toolchain rule
    /// is preferred, but native.cc_toolchain must exist for backwards compatibility.
    ///
    /// Currently a no-op stub that allows parsing.
    fn cc_toolchain<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = NoneOr::None)] _all_files: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] _toolchain_config: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] _toolchain_identifier: NoneOr<&str>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _visibility: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        _tags: UnpackListOrTuple<String>,
        #[starlark(kwargs)] _kwargs: Value<'v>,
    ) -> starlark::Result<NoneType> {
        // TODO(toolchains): Implement cc_toolchain target registration.
        // Currently a no-op stub for parsing compatibility.
        let _unused = name;
        Ok(NoneType)
    }
    /// The `glob()` function specifies a set of files using patterns.
    /// Bazel-compatible: can be called as `native.glob()` from .bzl files.
    ///
    /// A typical `glob` call looks like:
    /// ```python
    /// native.glob(["src/**/*.cc"], exclude = ["src/**/*_test.cc"])
    /// ```
    fn glob<'v>(
        include: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        exclude: UnpackListOrTuple<String>,
        #[starlark(require = named, default = true)] allow_empty: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueOfUnchecked<'v, UnpackList<String>>> {
        let _unused = allow_empty;
        let extra = ModuleInternals::from_context(eval, "native.glob")?;
        let spec = GlobSpec::new(&include.items, &exclude.items)?;
        let res = extra.resolve_glob(&spec).map(|path| path.as_str());
        Ok(eval.heap().alloc_typed_unchecked(AllocList(res)).cast())
    }

    /// Returns the name of the package being evaluated.
    /// Bazel-compatible: can be called as `native.package_name()` from .bzl files.
    ///
    /// For example, in the BUILD.bazel file `//foo/bar:BUILD.bazel`, this function
    /// returns `foo/bar`.
    fn package_name(eval: &mut Evaluator) -> starlark::Result<String> {
        Ok(BuildContext::from_context(eval)?
            .base_path()?
            .path()
            .to_string())
    }

    /// Returns the name of the repository the rule or build extension is called from.
    /// Bazel-compatible: can be called as `native.repository_name()` from .bzl files.
    ///
    /// The returned name starts with `@` for compatibility with Bazel.
    fn repository_name(eval: &mut Evaluator) -> starlark::Result<String> {
        Ok(format!(
            "@{}",
            BuildContext::from_context(eval)?.cell_info().name()
        ))
    }

    /// Returns a dict of all rules instantiated so far in the current BUILD file.
    /// Bazel-compatible: can be called as `native.existing_rules()` from .bzl files.
    ///
    /// The keys are rule names, and the values are dicts containing basic rule info.
    /// Note: Currently returns minimal information (name and kind). Full attribute
    /// introspection may be added in a future version.
    fn existing_rules<'v>(eval: &mut Evaluator<'v, '_, '_>) -> starlark::Result<Value<'v>> {
        let internals = ModuleInternals::from_context(eval, "native.existing_rules")?;
        let target_names = internals.get_target_names();

        let heap = eval.heap();
        let result: SmallMap<&str, Value<'v>> = target_names
            .iter()
            .map(|name| {
                // Return minimal dict with just the name for now
                // Full attribute introspection would require significant additional work
                let attrs_dict: SmallMap<&str, Value<'v>> =
                    [("name", heap.alloc(name.as_str()))].into_iter().collect();
                let attrs_val = heap.alloc(AllocDict(attrs_dict));
                (name.as_str(), attrs_val)
            })
            .collect();

        Ok(heap.alloc(AllocDict(result)))
    }

    /// Returns a dict of the attributes of the rule with the given name, or None if not found.
    /// Bazel-compatible: can be called as `native.existing_rule(name)` from .bzl files.
    ///
    /// Note: Currently returns minimal information. Full attribute introspection
    /// may be added in a future version.
    fn existing_rule<'v>(
        name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneOr<Value<'v>>> {
        let internals = ModuleInternals::from_context(eval, "native.existing_rule")?;

        if !internals.target_exists(name) {
            return Ok(NoneOr::None);
        }

        let heap = eval.heap();
        // Return minimal dict with just the name for now
        let attrs_dict: SmallMap<&str, Value<'v>> =
            [("name", heap.alloc(name))].into_iter().collect();
        Ok(NoneOr::Other(heap.alloc(AllocDict(attrs_dict))))
    }

    /// Converts a label string to a Label object relative to the current package.
    /// Bazel-compatible: can be called as `native.package_relative_label(label_string)` from .bzl files.
    ///
    /// For example, in package `//foo/bar`:
    /// - `native.package_relative_label(":target")` returns Label("//foo/bar:target")
    /// - `native.package_relative_label("//other:target")` returns Label("//other:target")
    fn package_relative_label<'v>(
        label_string: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StringValue<'v>> {
        let build_ctx = BuildContext::from_context(eval)?;
        let base_path = build_ctx.base_path()?;

        // If the label is already absolute (starts with // or @), return as-is
        // Otherwise, make it relative to the current package
        let resolved = if label_string.starts_with("//") || label_string.starts_with('@') {
            label_string.to_owned()
        } else if let Some(target) = label_string.strip_prefix(':') {
            // :target -> //package:target
            format!("//{}:{}", base_path.path(), target)
        } else {
            // target -> //package:target
            format!("//{}:{}", base_path.path(), label_string)
        };

        Ok(eval.heap().alloc_str(&resolved))
    }
}

/// Kuro's reported Bazel version for compatibility with modern rules.
///
/// This version is reported via `native.bazel_version` to satisfy version checks
/// in rulesets like rules_cc, bazel_features, etc. We report 9.0.0 to ensure
/// compatibility with Bazel 9.0+ rules.
///
/// Using "9.0.0" without a suffix so version comparisons work correctly.
/// The bazel_features module compares versions as tuples where released versions
/// (no prerelease suffix) compare greater than prereleases. This ensures checks
/// like `version >= "9.0.0-pre.20250911"` return True.
pub const KURO_BAZEL_VERSION: &str = "9.0.0";

/// Register the Bazel-compatible `native` namespace.
pub(crate) fn register_bazel_native(globals: &mut GlobalsBuilder) {
    globals.namespace("native", |registry| {
        bazel_native_module(registry);
        // Add bazel_version constant to the native module
        // This is accessed as `native.bazel_version` in Starlark
        registry.set("bazel_version", KURO_BAZEL_VERSION);
    });
}
