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
use crate::interpreter::globspec::GlobSpec;
use crate::interpreter::module_internals::ModuleInternals;

#[starlark_module]
pub(crate) fn register_module_natives(globals: &mut GlobalsBuilder) {
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
    fn existing_rules<'v>(
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
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
/// The "-kuro" suffix identifies this as Kuro rather than actual Bazel.
pub const KURO_BAZEL_VERSION: &str = "9.0.0-kuro";

/// Register the Bazel-compatible `native` namespace.
pub(crate) fn register_bazel_native(globals: &mut GlobalsBuilder) {
    globals.namespace("native", |registry| {
        bazel_native_module(registry);
        // Add bazel_version constant to the native module
        // This is accessed as `native.bazel_version` in Starlark
        registry.set("bazel_version", KURO_BAZEL_VERSION);
    });
}
