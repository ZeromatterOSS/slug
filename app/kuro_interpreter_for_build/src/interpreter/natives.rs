/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use anyhow::anyhow;
use kuro_build_api::interpreter::rule_defs::bazel_label::BazelLabel;
use kuro_build_api::interpreter::rule_defs::context::AnalysisActions;
use kuro_build_api::interpreter::rule_defs::depset::Depset;
use kuro_build_api::interpreter::rule_defs::depset::bazel_depset_tset_definition;
use kuro_build_api::interpreter::rule_defs::depset::depset_direct_and_transitive;
use kuro_build_api::interpreter::rule_defs::depset::make_depset_from_lists;
use kuro_build_api::interpreter::rule_defs::transitive_set::FrozenTransitiveSet;
use kuro_build_api::interpreter::rule_defs::transitive_set::FrozenTransitiveSetDefinition;
use kuro_build_api::interpreter::rule_defs::transitive_set::TransitiveSet;
use kuro_build_api::interpreter::rule_defs::transitive_set::TransitiveSetLike;
use kuro_build_api::interpreter::rule_defs::transitive_set::TransitiveSetOrdering;
use starlark::collections::SmallMap;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::FrozenValueTyped;
use starlark::values::StringValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::ValueOfUnchecked;
use starlark::values::ValueTyped;
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
use crate::macro_callable::StarlarkMacroCallable;

fn depset_to_transitive_set<'v>(
    depset: Value<'v>,
    actions: &AnalysisActions<'v>,
    definition: FrozenValueTyped<'v, FrozenTransitiveSetDefinition>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<ValueTyped<'v, TransitiveSet<'v>>> {
    let heap = eval.heap();
    let (direct, transitive) = depset_direct_and_transitive(depset, heap)?;
    let mut child_sets = Vec::new();

    for item in direct {
        let tset = {
            let mut state = actions.state()?;
            state.create_transitive_set(definition, Some(item), None, eval)?
        };
        child_sets.push(tset.to_value());
    }

    for child in transitive {
        let tset = depset_to_transitive_set(child, actions, definition, eval)?;
        child_sets.push(tset.to_value());
    }

    let children_value = if child_sets.is_empty() {
        None
    } else {
        Some(heap.alloc(AllocList(child_sets)).to_value())
    };

    let root = {
        let mut state = actions.state()?;
        state.create_transitive_set(definition, None, children_value, eval)?
    };
    Ok(root)
}

/// Convert a serde_json::Value to a Starlark Value on the given heap.
/// Used by existing_rules()/existing_rule() to convert CoercedAttr JSON representations
/// to Starlark values that can be returned to .bzl code.
pub(crate) fn json_to_starlark_value<'v>(
    heap: starlark::values::Heap<'v>,
    json: &serde_json::Value,
) -> Value<'v> {
    match json {
        serde_json::Value::Null => Value::new_none(),
        serde_json::Value::Bool(b) => Value::new_bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                heap.alloc(i as i32)
            } else if let Some(f) = n.as_f64() {
                heap.alloc(f)
            } else {
                heap.alloc(0)
            }
        }
        serde_json::Value::String(s) => heap.alloc_str(s).to_value(),
        serde_json::Value::Array(arr) => {
            let items: Vec<Value<'v>> = arr
                .iter()
                .map(|v| json_to_starlark_value(heap, v))
                .collect();
            heap.alloc(AllocList(items))
        }
        serde_json::Value::Object(map) => {
            // Check for select() representation: {"__type": "selector", "entries": {...}}
            if let Some(serde_json::Value::String(t)) = map.get("__type") {
                if t == "selector" {
                    // Return the entries as a dict for now (select() values show their resolved form)
                    if let Some(entries) = map.get("entries") {
                        return json_to_starlark_value(heap, entries);
                    }
                } else if t == "concat" {
                    // Concatenated selects - return first item as representative
                    if let Some(serde_json::Value::Array(items)) = map.get("items") {
                        if let Some(first) = items.first() {
                            return json_to_starlark_value(heap, first);
                        }
                    }
                }
            }
            let dict_entries: SmallMap<String, Value<'v>> = map
                .iter()
                .map(|(k, v)| (k.clone(), json_to_starlark_value(heap, v)))
                .collect();
            heap.alloc(AllocDict(dict_entries))
        }
    }
}

/// Extract cell name and package path from a project-relative filename.
///
/// During analysis, BuildContext is unavailable but we can determine the cell
/// from the call stack filename. The filename is a project-relative path:
/// - `bazel-external/{cell_name}+{version}/...` for external bzlmod modules
/// - `bazel-external/{canonical_name}/...` for extension/repository rule repos
/// - `bazel_tools/...` for bazel_tools cell
/// - Other paths for root cell
fn extract_cell_and_package_from_filename(filename: &str) -> (String, String) {
    if let Some(rest) = filename.strip_prefix("bazel-external/") {
        // bazel-external/{cell_name}+{version}/{cell_relative_path}
        // or bazel-external/{canonical_name}/{cell_relative_path}
        if let Some(dir_end) = rest.find('/') {
            let dir_name = &rest[..dir_end];
            // Extract cell name: strip "+version" suffix if present
            let cell_name = if let Some(plus_idx) = dir_name.find('+') {
                dir_name[..plus_idx].to_owned()
            } else {
                dir_name.to_owned()
            };
            let cell_relative = &rest[dir_end + 1..];
            if let Some(last_slash) = cell_relative.rfind('/') {
                return (cell_name, cell_relative[..last_slash].to_owned());
            }
            return (cell_name, String::new());
        }
    }
    if let Some(rest) = filename.strip_prefix("bazel_tools/") {
        if let Some(last_slash) = rest.rfind('/') {
            return ("bazel_tools".to_owned(), rest[..last_slash].to_owned());
        }
        return ("bazel_tools".to_owned(), String::new());
    }
    // Root cell or unknown - extract package from directory
    if let Some(last_slash) = filename.rfind('/') {
        return (String::new(), filename[..last_slash].to_owned());
    }
    (String::new(), String::new())
}

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
    ) -> starlark::Result<Value<'v>> {
        // In Bazel, Label() resolves relative to the file where it appears:
        // - In a .bzl file: // resolves to the .bzl file's repository
        // - In a BUILD file: // resolves to the BUILD file's repository
        // This is critical for rules packages (e.g., rules_cc) that use
        // Label("//:target") to refer to their own repo's targets.

        // Try BuildContext first (available during loading/interpretation).
        // During analysis, BuildContext is unavailable - fall back to call stack parsing.
        let (file_cell, pkg_path) = if let Ok(build_ctx) = BuildContext::from_context(eval) {
            let cell = build_ctx.starlark_path().cell().to_string();
            let pkg = match &build_ctx.additional {
                PerFileTypeContext::Build(module) => module
                    .buildfile_path()
                    .package()
                    .to_cell_path()
                    .path()
                    .as_str()
                    .to_owned(),
                PerFileTypeContext::Bzl(bzl_ctx) => {
                    bzl_ctx.bzl_path.path_parent().path().as_str().to_owned()
                }
                _ => build_ctx.base_path()?.path().as_str().to_owned(),
            };
            (cell, pkg)
        } else {
            // During analysis: extract cell name and package from call stack filename.
            // The filename is a project-relative path like:
            //   "bazel-external/rules_rust/0.40.0/rust/private/utils.bzl"
            //   "bazel_tools/tools/build_rules/filegroup.bzl"
            //   "some/local/file.bzl"
            let location = eval.call_stack_top_location();
            let filename = location.as_ref().map(|l| l.filename().to_owned());
            extract_cell_and_package_from_filename(filename.as_deref().unwrap_or(""))
        };

        let resolved = if label_string.starts_with("@@") || label_string.starts_with('@') {
            // Already fully qualified with repository
            label_string.to_owned()
        } else if label_string.starts_with("//") {
            // Absolute path within the current file's repository
            // Use @@ prefix for canonical labels (Bazel 9.0+ bzlmod format)
            format!("@@{}{}", file_cell, label_string)
        } else {
            // Relative label (:target or bare target)
            let target = label_string.strip_prefix(':').unwrap_or(label_string);
            format!("@@{}//{}:{}", file_cell, pkg_path, target)
        };

        Ok(eval.heap().alloc(BazelLabel::parse(&resolved)))
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

    /// Creates an analysis test transition.
    ///
    /// In Bazel, `analysis_test_transition` creates a configuration transition
    /// that can be used in analysis tests to modify build settings.
    ///
    /// Currently a stub that returns a struct with the settings stored.
    fn analysis_test_transition<'v>(
        #[starlark(require = named)] settings: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return a struct with settings stored, mimicking Bazel's transition object.
        // In Bazel, the returned object is passed to rule(cfg = ...) for analysis tests.
        let entries = vec![
            ("_type", eval.heap().alloc("analysis_test_transition")),
            ("settings", settings),
        ];
        Ok(eval.heap().alloc(AllocDict(entries)))
    }

    /// Bazel's `AnalysisTestResultInfo` provider for analysis test results.
    ///
    /// Used by bazel_skylib's unittest framework to return test results.
    fn AnalysisTestResultInfo<'v>(
        #[starlark(require = named, default = false)] success: bool,
        #[starlark(require = named, default = "")] message: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let entries = vec![
            ("success", eval.heap().alloc(success)),
            ("message", eval.heap().alloc(message)),
        ];
        Ok(eval.heap().alloc(AllocDict(entries)))
    }

    /// Bazel 8.0+ symbolic macro definition.
    ///
    /// In Bazel, `macro()` defines a symbolic macro, similar to `rule()` but for macros.
    /// Symbolic macros provide better introspection than legacy macros (function-based macros).
    ///
    /// Returns a callable `MacroCallable` that, when invoked in BUILD files, calls the
    /// implementation function with the provided arguments (name, visibility, attrs).
    ///
    /// See: https://bazel.build/rules/lib/globals/bzl#macro
    fn r#macro<'v>(
        #[starlark(require = named)] implementation: Value<'v>,
        #[starlark(require = named, default = NoneOr::None)] doc: NoneOr<&str>,
        #[starlark(require = named, default = NoneOr::None)] attrs: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] inherit_attrs: NoneOr<Value<'v>>,
        #[starlark(require = named, default = false)] finalizer: bool,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkMacroCallable<'v>> {
        // TODO(macro): Wire attrs/inherit_attrs for attribute validation.
        let _ = (attrs, inherit_attrs);
        let doc_str = match doc {
            NoneOr::Other(d) if !d.is_empty() => Some(d.to_owned()),
            _ => None,
        };
        Ok(StarlarkMacroCallable::new(
            implementation,
            finalizer,
            doc_str,
        ))
    }

    /// Bazel's `AnalysisFailureInfo` provider for analysis failure detection.
    ///
    /// Used by bazel_skylib's unittest framework to detect analysis failures.
    /// In Bazel, this is a callable provider: AnalysisFailureInfo(causes=depset(...))
    fn AnalysisFailureInfo<'v>(
        #[starlark(require = named, default = NoneOr::None)] causes: NoneOr<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let entries = vec![
            ("_type", eval.heap().alloc("AnalysisFailureInfo")),
            (
                "causes",
                match causes {
                    NoneOr::Other(v) => v,
                    NoneOr::None => Value::new_none(),
                },
            ),
        ];
        Ok(eval.heap().alloc(AllocDict(entries)))
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

    // Note: toolchain() is implemented in native_rules.rs as a proper native rule
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
    // Note: filegroup() is implemented in native_rules.rs as a proper native rule

    /// Legacy cc_toolchain_suite rule (BUILD file version).
    ///
    /// This is a Bazel built-in native rule that was used before toolchain resolution.
    /// In modern Bazel (and rules_cc 0.2.16+), this is deprecated in favor of toolchain()
    /// rules, but native cc_toolchain_suite must still exist for backwards compatibility.
    fn cc_toolchain_suite<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "cc_toolchain_suite")?;
        let target_node = crate::interpreter::native_rules::create_native_target_node(
            crate::interpreter::native_rules::rule_defs::CC_TOOLCHAIN_SUITE_RULE.clone(),
            internals.package(),
            name,
            vec![],
            &crate::interpreter::native_rules::extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;
        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Legacy cc_toolchain rule (BUILD file version).
    ///
    /// This is a Bazel built-in native rule for C++ toolchain definition.
    /// In modern Bazel with rules_cc 0.2.16+, the pure Starlark cc_toolchain rule
    /// is preferred, but native cc_toolchain must exist for backwards compatibility.
    fn cc_toolchain<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "cc_toolchain")?;
        let target_node = crate::interpreter::native_rules::create_native_target_node(
            crate::interpreter::native_rules::rule_defs::CC_TOOLCHAIN_RULE.clone(),
            internals.package(),
            name,
            vec![],
            &crate::interpreter::native_rules::extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;
        internals.record(target_node)?;
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
    fn cc_toolchain_suite<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "cc_toolchain_suite")?;
        let target_node = crate::interpreter::native_rules::create_native_target_node(
            crate::interpreter::native_rules::rule_defs::CC_TOOLCHAIN_SUITE_RULE.clone(),
            internals.package(),
            name,
            vec![],
            &crate::interpreter::native_rules::extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;
        internals.record(target_node)?;
        Ok(NoneType)
    }

    /// Legacy native cc_toolchain rule.
    ///
    /// This is a Bazel built-in native rule for C++ toolchain definition.
    /// In modern Bazel with rules_cc 0.2.16+, the pure Starlark cc_toolchain rule
    /// is preferred, but native.cc_toolchain must exist for backwards compatibility.
    fn cc_toolchain<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let internals = ModuleInternals::from_context(eval, "cc_toolchain")?;
        let target_node = crate::interpreter::native_rules::create_native_target_node(
            crate::interpreter::native_rules::rule_defs::CC_TOOLCHAIN_RULE.clone(),
            internals.package(),
            name,
            vec![],
            &crate::interpreter::native_rules::extract_visibility_strings(visibility),
            internals.attr_coercion_context(),
            &internals.default_visibility(),
        )?;
        internals.record(target_node)?;
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
        // Bazel-compatible parameter: 1 = exclude directories (default), 0 = include directories
        #[starlark(require = named, default = 1)] exclude_directories: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueOfUnchecked<'v, UnpackList<String>>> {
        let _ = exclude_directories;
        let extra = ModuleInternals::from_context(eval, "native.glob")?;
        let spec = GlobSpec::new(&include.items, &exclude.items)?;
        if !allow_empty {
            let results: Vec<_> = extra
                .resolve_glob(&spec)
                .map(|path| path.as_str().to_owned())
                .collect();
            if results.is_empty() {
                return Err(starlark::Error::new_other(anyhow::anyhow!(
                    "glob pattern '{}' didn't match anything, but allow_empty is set to False (the default value of allow_empty can be set with package(default_glob_allow_empty = ...))",
                    include.items.join(", ")
                )));
            }
            Ok(eval
                .heap()
                .alloc_typed_unchecked(AllocList(results.iter().map(|s| s.as_str())))
                .cast())
        } else {
            let res = extra.resolve_glob(&spec).map(|path| path.as_str());
            Ok(eval.heap().alloc_typed_unchecked(AllocList(res)).cast())
        }
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

    /// Returns the name of the Bazel module associated with the repository where
    /// the calling .bzl file lives. For the root module this returns the module name
    /// from MODULE.bazel; for external modules it returns their declared name.
    ///
    /// If called outside a bzlmod context, returns an empty string.
    ///
    /// See: https://bazel.build/rules/lib/builtins/native#module_name
    fn module_name(eval: &mut Evaluator) -> starlark::Result<NoneOr<String>> {
        let cell_name = BuildContext::from_context(eval)?
            .cell_info()
            .name()
            .to_string();
        if kuro_core::cells::is_root_cell_name(&cell_name) {
            // For root cell, return the cell name from MODULE.bazel
            // (CellResolver sets it to the module(name=...) value or "_main")
            Ok(NoneOr::Other(cell_name))
        } else {
            Ok(NoneOr::Other(cell_name))
        }
    }

    /// Returns the version of the Bazel module associated with the repository where
    /// the calling .bzl file lives.
    ///
    /// See: https://bazel.build/rules/lib/builtins/native#module_version
    fn module_version(eval: &mut Evaluator) -> starlark::Result<NoneOr<String>> {
        let cell_name = BuildContext::from_context(eval)?
            .cell_info()
            .name()
            .to_string();
        match kuro_bzlmod::get_module_version(&cell_name) {
            Some(version) => Ok(NoneOr::Other(version)),
            None => Ok(NoneOr::None),
        }
    }

    /// Convert a depset to a transitive_set.
    ///
    /// This is a Kuro-specific bridge to preserve transitive_set performance internally
    /// while exposing Bazel-compatible depset APIs to rules.
    fn transitive_set_from_depset<'v>(
        depset: Value<'v>,
        #[starlark(require = named, default = "default")] order: &str,
        #[starlark(require = named, default = NoneOr::None)] actions: NoneOr<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        match order {
            "default" | "preorder" | "postorder" | "topological" => {}
            _ => {
                return Err(starlark::Error::new_other(anyhow!(
                    "expected order to be one of `default`, `preorder`, `postorder`, `topological`, got `{order}`"
                )));
            }
        }
        let actions = match actions {
            NoneOr::None => {
                return Err(starlark::Error::new_other(anyhow!(
                    "native.transitive_set_from_depset requires actions=ctx.actions"
                )));
            }
            NoneOr::Other(value) => value,
        };
        let Some(actions) = actions.downcast_ref::<AnalysisActions>() else {
            return Err(starlark::Error::new_other(anyhow!(
                "actions must be an AnalysisActions instance"
            )));
        };

        let definition = bazel_depset_tset_definition()?;
        let definition = definition.owned_frozen_value_typed(eval.frozen_heap());
        let tset = depset_to_transitive_set(depset, actions, definition, eval)?;
        Ok(tset.to_value())
    }

    /// Convert a transitive_set to a depset by materializing its traversal.
    fn depset_from_transitive_set<'v>(
        tset: Value<'v>,
        #[starlark(require = named, default = "default")] order: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let ordering = match order {
            "default" => TransitiveSetOrdering::Bfs,
            "preorder" => TransitiveSetOrdering::Preorder,
            "postorder" => TransitiveSetOrdering::Postorder,
            "topological" => TransitiveSetOrdering::Topological,
            _ => {
                return Err(starlark::Error::new_other(anyhow!(
                    "expected order to be one of `default`, `preorder`, `postorder`, `topological`, got `{order}`"
                )));
            }
        };

        let mut values = Vec::new();
        if let Some(transitive) = TransitiveSet::from_value(tset) {
            for value in transitive.iter_values(ordering)? {
                values.push(value);
            }
        } else if let Some(transitive) = FrozenTransitiveSet::from_value(tset) {
            for value in transitive.iter_values(ordering)? {
                values.push(value);
            }
        } else if tset.downcast_ref::<Depset>().is_some() {
            // Already a depset; return it directly.
            return Ok(tset);
        } else {
            return Err(starlark::Error::new_other(anyhow!(
                "depset_from_transitive_set expects a transitive_set"
            )));
        }

        let heap = eval.heap();
        make_depset_from_lists(heap, values, Vec::new(), order)
    }

    /// Returns the name of the repository the rule or build extension is called from.
    /// Bazel-compatible: can be called as `native.repository_name()` from .bzl files.
    ///
    /// The returned name starts with `@` for compatibility with Bazel.
    /// For the main repository, returns `@` (empty after @), matching Bazel behavior.
    fn repository_name(eval: &mut Evaluator) -> starlark::Result<String> {
        let cell_name = BuildContext::from_context(eval)?
            .cell_info()
            .name()
            .to_string();
        if kuro_core::cells::is_root_cell_name(&cell_name) {
            Ok("@".to_owned())
        } else {
            Ok(format!("@{}", cell_name))
        }
    }

    /// Returns a dict of all rules instantiated so far in the current BUILD file.
    /// Bazel-compatible: can be called as `native.existing_rules()` from .bzl files.
    ///
    /// The keys are rule names, and the values are dicts containing all rule attributes
    /// plus synthetic "name" and "kind" fields.
    fn existing_rules<'v>(eval: &mut Evaluator<'v, '_, '_>) -> starlark::Result<Value<'v>> {
        // When called outside a BUILD file (e.g., from a module extension via maybe()),
        // return an empty dict so that maybe() always proceeds to create the repo.
        let Ok(internals) = ModuleInternals::from_context(eval, "native.existing_rules") else {
            return Ok(eval.heap().alloc(AllocDict(SmallMap::<&str, Value>::new())));
        };
        let targets = internals.get_targets_with_attrs();

        let heap = eval.heap();
        let result: SmallMap<String, Value<'v>> = targets
            .into_iter()
            .map(|(name, kind, attrs)| {
                let mut attrs_dict: SmallMap<String, Value<'v>> = SmallMap::new();
                attrs_dict.insert("name".to_owned(), heap.alloc(name.as_str()));
                attrs_dict.insert("kind".to_owned(), heap.alloc(kind.as_str()));
                for (attr_name, json_val) in attrs {
                    // Skip "name" since we already added it
                    if attr_name == "name" {
                        continue;
                    }
                    attrs_dict.insert(attr_name, json_to_starlark_value(heap, &json_val));
                }
                let attrs_val = heap.alloc(AllocDict(attrs_dict));
                (name, attrs_val)
            })
            .collect();

        Ok(heap.alloc(AllocDict(result)))
    }

    /// Returns a dict of the attributes of the rule with the given name, or None if not found.
    /// Bazel-compatible: can be called as `native.existing_rule(name)` from .bzl files.
    ///
    /// Returns all explicitly-set attributes plus synthetic "name" and "kind" fields.
    fn existing_rule<'v>(
        name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneOr<Value<'v>>> {
        // When called outside a BUILD file context, return None (repo doesn't exist yet).
        let Ok(internals) = ModuleInternals::from_context(eval, "native.existing_rule") else {
            return Ok(NoneOr::None);
        };

        let (kind, attrs) = match internals.get_target_with_attrs(name) {
            Some(data) => data,
            None => return Ok(NoneOr::None),
        };

        let heap = eval.heap();
        let mut attrs_dict: SmallMap<String, Value<'v>> = SmallMap::new();
        attrs_dict.insert("name".to_owned(), heap.alloc(name));
        attrs_dict.insert("kind".to_owned(), heap.alloc(kind.as_str()));
        for (attr_name, json_val) in attrs {
            if attr_name == "name" {
                continue;
            }
            attrs_dict.insert(attr_name, json_to_starlark_value(heap, &json_val));
        }
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
        // Also include native rules (alias, config_setting, constraint_setting, etc.)
        // so they can be called as native.alias(), native.config_setting(), etc. from .bzl files
        crate::interpreter::native_rules::register_native_rules(registry);
        // Add bazel_version constant to the native module
        // This is accessed as `native.bazel_version` in Starlark
        registry.set("bazel_version", KURO_BAZEL_VERSION);
    });
}
