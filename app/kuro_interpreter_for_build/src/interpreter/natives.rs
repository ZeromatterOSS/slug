/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use kuro_build_api::interpreter::rule_defs::bazel_label::BazelLabel;
use kuro_build_api::interpreter::rule_defs::context::AnalysisActions;
use kuro_build_api::interpreter::rule_defs::depset::bazel_depset_tset_definition;
use kuro_build_api::interpreter::rule_defs::depset::depset_direct_and_transitive;
use kuro_build_api::interpreter::rule_defs::depset::is_depset_value;
use kuro_build_api::interpreter::rule_defs::depset::make_depset_from_lists;
use kuro_build_api::interpreter::rule_defs::py_common::AnalysisTestResultInfoProvider;
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
use starlark::values::ValueIdentity;
use starlark::values::ValueLike;
use starlark::values::ValueOfUnchecked;
use starlark::values::dict::AllocDict;
use starlark::values::list::AllocList;
use starlark::values::list::UnpackList;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::tuple::AllocTuple;

use crate::interpreter::build_context::BuildContext;
use crate::interpreter::build_context::PerFileTypeContext;
use crate::interpreter::globspec::GlobSpec;
use crate::interpreter::module_internals::ModuleInternals;
use crate::macro_callable::StarlarkMacroCallable;

fn depset_to_transitive_set<'v>(
    depset: Value<'v>,
    actions: &AnalysisActions<'v>,
    definition: FrozenValueTyped<'v, FrozenTransitiveSetDefinition>,
    cache: &mut std::collections::HashMap<ValueIdentity<'v>, Value<'v>>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    let heap = eval.heap();
    let identity = depset.identity();
    if let Some(tset) = cache.get(&identity) {
        return Ok(*tset);
    }

    let (direct, transitive) = depset_direct_and_transitive(depset, heap)?;
    let mut child_sets = Vec::new();

    for child in transitive {
        child_sets.push(depset_to_transitive_set(
            child, actions, definition, cache, eval,
        )?);
    }

    let children_value = if child_sets.is_empty() {
        None
    } else {
        Some(heap.alloc(AllocList(child_sets)).to_value())
    };

    // `TransitiveSet` nodes have zero or one value, while Bazel depset nodes
    // have zero or more direct values. For the internal BazelDepsetTset bridge,
    // the node value is an immutable tuple of that depset node's direct values.
    // The reverse bridge expands this tuple again; ordinary tset traversal still
    // sees it as one node value, so this remains a lossy Kuro-specific bridge
    // rather than a public alias between depset and TransitiveSet.
    let direct_value = heap.alloc(AllocTuple(direct)).to_value();
    let tset = {
        let mut state = actions.state()?;
        state.create_transitive_set(definition, Some(direct_value), children_value, eval)?
    };
    let tset = tset.to_value();
    cache.insert(identity, tset);
    Ok(tset)
}

fn parse_depset_from_transitive_set_order(
    order: &str,
    bazel_depset_tset: bool,
) -> starlark::Result<TransitiveSetOrdering> {
    match order {
        // For the internal depset-shaped tset bridge, `default` must preserve
        // Bazel depset default flattening, which is postorder-like in the
        // Bazel 9.1.0 probes. Keep the legacy generic tset bridge behavior for
        // non-BazelDepsetTset inputs.
        "default" if bazel_depset_tset => Ok(TransitiveSetOrdering::Postorder),
        "default" => Ok(TransitiveSetOrdering::Bfs),
        "preorder" => Ok(TransitiveSetOrdering::Preorder),
        "postorder" => Ok(TransitiveSetOrdering::Postorder),
        "topological" => Ok(TransitiveSetOrdering::Topological),
        _ => Err(kuro_error::kuro_error!(
            kuro_error::ErrorTag::Input,
            "expected order to be one of `default`, `preorder`, `postorder`, `topological`, got `{order}`"
        )
        .into()),
    }
}

fn append_bazel_depset_tset_values<'v>(
    values: &mut Vec<Value<'v>>,
    direct_tuple: Value<'v>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<()> {
    let direct = direct_tuple.iterate(eval.heap())?;
    values.extend(direct);
    Ok(())
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
        // bazel-external/{dir_name}/{cell_relative_path}
        //
        // dir_name can take several shapes:
        //   - "{cell_name}"                                 -- use_repo_rule-style
        //     (e.g. "llvm-project")
        //   - "{cell_name}+{version}"                       -- bzlmod module cell
        //     (e.g. "rules_cc+0.2.17" -> cell_name = "rules_cc")
        //   - "{owner}+{extension}+{repo_name}"             -- module-extension repo
        //     (e.g. "_main+llvm_repos_extension+llvm-raw"
        //      -> cell_name = "llvm-raw"; taking the first `+` segment would
        //      yield "_main" which is the root-module canonical prefix, not
        //      the apparent repo name used in Label() resolution)
        if let Some(dir_end) = rest.find('/') {
            let dir_name = &rest[..dir_end];
            let plus_count = dir_name.matches('+').count();
            let cell_name = match plus_count {
                0 => dir_name.to_owned(),
                1 => {
                    // "{cell_name}+{version}"
                    dir_name[..dir_name.find('+').unwrap()].to_owned()
                }
                _ => {
                    // "{owner}+{extension}+{repo_name}[+...]"
                    dir_name[dir_name.rfind('+').unwrap() + 1..].to_owned()
                }
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

fn canonical_repo_name_for_label_context(
    eval: &Evaluator<'_, '_, '_>,
    apparent_repo_name: &str,
) -> String {
    if apparent_repo_name.is_empty() {
        return String::new();
    }

    if let Ok(build_ctx) = BuildContext::from_context(eval) {
        if let Ok(cell_name) = build_ctx
            .cell_info()
            .cell_alias_resolver()
            .resolve(apparent_repo_name)
        {
            let cell_name = cell_name.as_str();
            if kuro_core::cells::is_root_cell_name(cell_name) {
                return String::new();
            }
            return cell_name.to_owned();
        }
    }

    if let Some(canonical_name) =
        kuro_core::cells::resolve_dynamic_extension_cell_alias(apparent_repo_name)
    {
        if kuro_core::cells::is_root_cell_name(&canonical_name) {
            return String::new();
        }
        return canonical_name;
    }

    apparent_repo_name.to_owned()
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

        // Bazel resolves `Label("//...")` relative to the LEXICAL location of
        // the call, i.e. the file containing the Label() call as written —
        // not the file currently being interpreted. When a macro defined in
        // `@rules_cc//cc/toolchains/toolchain.bzl` calls `Label("//cc/...")`
        // and that macro is invoked from `@llvm_toolchains//:BUILD.bazel`,
        // the label must resolve in `@rules_cc`, not `@llvm_toolchains`.
        //
        // `BuildContext::from_context(eval)` reports the *currently
        // interpreting* file's cell, which is the BUILD file in the macro
        // case — wrong. Use the call-stack top location's filename to find
        // the .bzl file that lexically contains the Label() call, and
        // derive the cell from its project-relative path. Only fall back
        // to BuildContext when the call stack lacks a location (rare).
        let (file_cell, pkg_path) = {
            let location = eval.call_stack_top_location();
            let filename = location.as_ref().map(|l| l.filename().to_owned());
            match filename.as_deref() {
                Some(f) if !f.is_empty() => extract_cell_and_package_from_filename(f),
                _ => {
                    if let Ok(build_ctx) = BuildContext::from_context(eval) {
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
                        (String::new(), String::new())
                    }
                }
            }
        };

        let current_repo = canonical_repo_name_for_label_context(eval, &file_cell);
        let resolved = kuro_bzlmod::canonicalize_label_with_package_context_and_repo_resolver(
            label_string,
            current_repo,
            &pkg_path,
            None,
            |apparent_repo| {
                Some(kuro_bzlmod::CanonicalRepoName::new(
                    canonical_repo_name_for_label_context(eval, apparent_repo),
                ))
            },
        )
        .map(|label| label.to_unambiguous_string())
        .unwrap_or_else(|| label_string.to_owned());

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
    const AnalysisTestResultInfo: AnalysisTestResultInfoProvider = AnalysisTestResultInfoProvider;

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
        let doc_str = match doc {
            NoneOr::Other(d) if !d.is_empty() => Some(d.to_owned()),
            _ => None,
        };
        let attrs_val = match attrs {
            NoneOr::Other(v) => Some(v),
            NoneOr::None => None,
        };
        let inherit_attrs_val = match inherit_attrs {
            NoneOr::Other(v) => Some(v),
            NoneOr::None => None,
        };
        Ok(StarlarkMacroCallable::new(
            implementation,
            finalizer,
            doc_str,
            attrs_val,
            inherit_attrs_val,
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

    /// `cc_toolchain_suite` was removed in Bazel 9. Loaded as a stub so the
    /// diagnostic surfaces during analysis. rules_cc 0.2.16's
    /// `cc_toolchain_suite` wrapper still calls `native.cc_toolchain_suite`,
    /// which means the diagnostic will reach users via that wrapper —
    /// consistent with Bazel 9 behavior. See Plan 27.2.
    fn cc_toolchain_suite<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        crate::interpreter::native_rules::register_removed_rule(
            crate::interpreter::native_rules::rule_defs::REMOVED_CC_TOOLCHAIN_SUITE_RULE.clone(),
            "cc_toolchain_suite",
            name,
            visibility,
            eval,
        )
    }

    /// `cc_toolchain` was removed in Bazel 9. See `cc_toolchain_suite`.
    fn cc_toolchain<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        crate::interpreter::native_rules::register_removed_rule(
            crate::interpreter::native_rules::rule_defs::REMOVED_CC_TOOLCHAIN_RULE.clone(),
            "cc_toolchain",
            name,
            visibility,
            eval,
        )
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
    /// `native.cc_toolchain_suite` was removed in Bazel 9. Loaded as a stub
    /// so the diagnostic surfaces during analysis. rules_cc 0.2.16's wrapper
    /// calls this function, which means the diagnostic will reach users via
    /// the wrapper — consistent with Bazel 9 behavior. See Plan 27.2.
    fn cc_toolchain_suite<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        crate::interpreter::native_rules::register_removed_rule(
            crate::interpreter::native_rules::rule_defs::REMOVED_CC_TOOLCHAIN_SUITE_RULE.clone(),
            "cc_toolchain_suite",
            name,
            visibility,
            eval,
        )
    }

    /// `native.cc_toolchain` was removed in Bazel 9. See `native.cc_toolchain_suite`.
    fn cc_toolchain<'v>(
        #[starlark(require = named)] name: &str,
        #[starlark(require = named, default = starlark::values::none::NoneType)] visibility: Value<
            'v,
        >,
        #[starlark(kwargs)] _extra_kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        crate::interpreter::native_rules::register_removed_rule(
            crate::interpreter::native_rules::rule_defs::REMOVED_CC_TOOLCHAIN_RULE.clone(),
            "cc_toolchain",
            name,
            visibility,
            eval,
        )
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
        let extra = ModuleInternals::from_context(eval, "native.glob")?;
        let spec = GlobSpec::new(&include.items, &exclude.items)?;

        let mut results: Vec<String> = extra
            .resolve_glob(&spec)
            .map(|path| path.as_str().to_owned())
            .collect();

        // Bazel default: directories are excluded. When the caller passes
        // `exclude_directories = 0` (e.g. rules_cc's llvm toolchain BUILD
        // does this to grab the versioned `lib/clang/<N>` directory), also
        // include matching directory paths derived from the files in this
        // package's listing. The kuro `PackageFileListing` only stores
        // files, so directory entries must be reconstructed from each
        // file's parent chain.
        if exclude_directories == 0 {
            let mut seen: std::collections::HashSet<String> = results.iter().cloned().collect();
            let mut dir_matches: Vec<String> = Vec::new();
            for file in extra.glob_directory_candidates() {
                let mut current = file.parent();
                while let Some(dir) = current {
                    if dir.as_str().is_empty() {
                        break;
                    }
                    if !seen.insert(dir.as_str().to_owned()) {
                        // Already seen this dir (or it was a file match) —
                        // and all of its ancestors have been visited too.
                        break;
                    }
                    if spec.matches(dir.as_str()) {
                        dir_matches.push(dir.as_str().to_owned());
                    }
                    current = dir.parent();
                }
            }
            results.extend(dir_matches);
        }

        if !allow_empty && results.is_empty() {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "glob pattern '{}' didn't match anything, but allow_empty is set to False (the default value of allow_empty can be set with package(default_glob_allow_empty = ...))",
                include.items.join(", ")
            )
            .into());
        }

        Ok(eval
            .heap()
            .alloc_typed_unchecked(AllocList(results.iter().map(|s| s.as_str())))
            .cast())
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
    /// Kuro-specific bridge. This is not part of Bazel's public `native`
    /// surface and is lossy: the returned internal `BazelDepsetTset` preserves
    /// depset node shape and shared child identity within this conversion call,
    /// but stores each depset node's direct values as one tuple-valued tset
    /// node. It does not make depset a public alias for `TransitiveSet`.
    fn transitive_set_from_depset<'v>(
        depset: Value<'v>,
        #[starlark(require = named, default = "default")] order: &str,
        #[starlark(require = named, default = NoneOr::None)] actions: NoneOr<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        match order {
            "default" | "preorder" | "postorder" | "topological" => {}
            _ => {
                return Err(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "expected order to be one of `default`, `preorder`, `postorder`, `topological`, got `{order}`"
                )
                .into());
            }
        }
        let actions = match actions {
            NoneOr::None => {
                return Err(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Input,
                    "native.transitive_set_from_depset requires actions=ctx.actions"
                )
                .into());
            }
            NoneOr::Other(value) => value,
        };
        let Some(actions) = actions.downcast_ref::<AnalysisActions>() else {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "actions must be an AnalysisActions instance"
            )
            .into());
        };

        let definition = bazel_depset_tset_definition()?;
        let definition = definition.owned_frozen_value_typed(eval.frozen_heap());
        let mut cache = std::collections::HashMap::new();
        depset_to_transitive_set(depset, actions, definition, &mut cache, eval)
    }

    /// Convert a transitive_set to a depset by materializing its traversal.
    ///
    /// This is a lossy Kuro-specific bridge: projections, reductions,
    /// definition identity, and transitive_set node identity are not preserved.
    /// For the internal `BazelDepsetTset` shape created by
    /// `native.transitive_set_from_depset`, tuple-valued node payloads are
    /// expanded back to direct depset values before constructing the depset.
    fn depset_from_transitive_set<'v>(
        tset: Value<'v>,
        #[starlark(require = named, default = "default")] order: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let mut values = Vec::new();
        if let Some(transitive) = TransitiveSet::from_value(tset) {
            let bazel_depset_tset = transitive.definition_name() == "BazelDepsetTset";
            let ordering = parse_depset_from_transitive_set_order(order, bazel_depset_tset)?;
            for value in transitive.iter_values(ordering)? {
                if bazel_depset_tset {
                    append_bazel_depset_tset_values(&mut values, value, eval)?;
                } else {
                    values.push(value);
                }
            }
        } else if let Some(transitive) = FrozenTransitiveSet::from_value(tset) {
            let bazel_depset_tset = transitive.definition_name() == "BazelDepsetTset";
            let ordering = parse_depset_from_transitive_set_order(order, bazel_depset_tset)?;
            for value in transitive.iter_values(ordering)? {
                if bazel_depset_tset {
                    append_bazel_depset_tset_values(&mut values, value, eval)?;
                } else {
                    values.push(value);
                }
            }
        } else if is_depset_value(tset) {
            // Already a depset; return it directly.
            return Ok(tset);
        } else {
            return Err(kuro_error::kuro_error!(
                kuro_error::ErrorTag::Input,
                "depset_from_transitive_set expects a transitive_set"
            )
            .into());
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
/// in rulesets like rules_cc, bazel_features, etc. We report 9.0.1 to ensure
/// compatibility with Bazel 9.0+ rules and to flip
/// `bazel_features.external_deps.repo_rules_relativize_symlinks` to True so
/// rules_rs's `relative_symlink` helper takes the `rctx.symlink` branch (which
/// kuro implements correctly with absolute paths) instead of the
/// `ln -sf relative_file(...)` fallback (which depends on a fully-loaded
/// `bazel_lib` and produces broken `var/...`-prefixed targets in kuro).
///
/// Using "9.0.1" without a suffix so version comparisons work correctly.
/// The bazel_features module compares versions as tuples where released versions
/// (no prerelease suffix) compare greater than prereleases. This ensures checks
/// like `version >= "9.0.0-pre.20250911"` return True.
pub const KURO_BAZEL_VERSION: &str = "9.0.1";

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
