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

use crate::interpreter::build_context::BuildContext;
use crate::interpreter::globspec::GlobSpec;
use crate::interpreter::module_internals::ModuleInternals;

#[starlark_module]
pub(crate) fn register_path(builder: &mut GlobalsBuilder) {
    /// The `glob()` function specifies a set of files using patterns.
    /// Only available from `BUCK` files.
    ///
    /// A typical `glob` call looks like:
    ///
    /// ```python
    /// glob(["foo/**/*.h"])
    /// ```
    ///
    /// This call will match all header files in the `foo` directory, recursively.
    ///
    /// You can also pass a named `exclude` parameter to remove files matching a pattern:
    ///
    /// ```python
    /// glob(["foo/**/*.h"], exclude = ["**/config.h"])
    /// ```
    ///
    /// This call will remove all `config.h` files from the initial match.
    ///
    /// The `glob()` call is evaluated against the list of files owned by this `BUCK` file.
    /// A file is owned by whichever `BUCK` file is closest above it - so given `foo/BUCK` and
    /// `foo/bar/BUCK` the file `foo/file.txt` would be owned by `foo/BUCK` (and available from
    /// its `glob` results) but the file `foo/bar/file.txt` would be owned by `foo/bar/BUCk`
    /// and _not_ appear in the glob result of `foo/BUCK`, even if you write `glob(["bar/file.txt"])`.
    /// As a consequence of this rule, `glob(["../foo.txt"])` will always return an empty list of files.
    ///
    /// `glob` is evaluated case-sensitively, matching Bazel behavior.
    fn glob<'v>(
        include: UnpackListOrTuple<String>,
        #[starlark(require = named, default=UnpackListOrTuple::default())]
        exclude: UnpackListOrTuple<String>,
        #[starlark(require = named, default = true)] allow_empty: bool,
        // Bazel-compatible parameter: 1 = exclude directories (default), 0 = include directories
        #[starlark(require = named, default = 1)] exclude_directories: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueOfUnchecked<'v, UnpackList<String>>> {
        let _ = exclude_directories;
        let extra = ModuleInternals::from_context(eval, "glob")?;
        let spec = GlobSpec::new(&include.items, &exclude.items)?;
        if !allow_empty {
            // Collect results to check emptiness
            let results: Vec<_> = extra
                .resolve_glob(&spec)
                .map(|path| path.as_str().to_owned())
                .collect();
            if results.is_empty() {
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
        } else {
            let res = extra.resolve_glob(&spec).map(|path| path.as_str());
            Ok(eval.heap().alloc_typed_unchecked(AllocList(res)).cast())
        }
    }

    /// `package_name()` can only be called in buildfiles (e.g. BUCK files) or PACKAGE files, and returns the name of the package.
    /// E.g. inside `foo//bar/baz/BUCK` the output will be `bar/baz`.
    /// E.g. inside `foo//bar/PACKAGE` the output will be `bar`.
    fn package_name(eval: &mut Evaluator) -> starlark::Result<String> {
        // An (IMO) unfortunate choice in the skylark api is that this just gives the cell-relative
        //  path of the package (which isn't a unique "name" for the package)
        Ok(BuildContext::from_context(eval)?
            .base_path()?
            .path()
            .to_string())
    }

    /// `get_base_path()` can only be called in buildfiles (e.g. BUCK files) or PACKAGE files, and returns the name of the package.
    /// E.g. inside `foo//bar/baz/BUCK` the output will be `bar/baz`.
    /// E.g. inside `foo//bar/PACKAGE` the output will be `bar`.
    ///
    /// This function is identical to `package_name`.
    fn get_base_path(eval: &mut Evaluator) -> starlark::Result<String> {
        Ok(BuildContext::from_context(eval)?
            .base_path()?
            .path()
            .to_string())
    }

    /// Returns the canonical name of the repository the rule or BUILD extension is called from.
    ///
    /// Bazel-compatible: `repo_name()` is available in BUILD files and .bzl files.
    ///
    /// For the root repository, returns `""` (empty string).
    /// For external repositories, returns the canonical name (e.g. `"rules_cc~0.0.16"`).
    ///
    /// See: https://bazel.build/rules/lib/globals/build#repo_name
    fn repo_name(eval: &mut Evaluator) -> starlark::Result<String> {
        let cell_name = BuildContext::from_context(eval)?.cell_info().name().name();
        let name_str = cell_name.as_str();
        // In Bazel, the root repository has repo_name() == "" (empty string).
        // External repos return their canonical name.
        if kuro_core::cells::is_root_cell_name(name_str) {
            Ok(String::new())
        } else {
            Ok(name_str.to_owned())
        }
    }

    /// Like `get_cell_name()` but prepends a leading `@` for compatibility with Buck1.
    /// You should call `get_cell_name()` instead, and if you really want the `@`,
    /// prepend it yourself.
    fn repository_name(eval: &mut Evaluator) -> starlark::Result<String> {
        // In Bazel, repository_name() returns "@" for the root repository
        // and "@<repo_name>" for external repositories.
        // In practice, most users do `repository_name()[1:]` to drop the leading `@`.
        let cell_name = BuildContext::from_context(eval)?.cell_info().name().name();
        let name_str = cell_name.as_str();
        if kuro_core::cells::is_root_cell_name(name_str) {
            Ok("@".to_owned())
        } else {
            Ok(format!("@{}", name_str))
        }
    }

    /// Returns the name of the Bazel module associated with the repository where
    /// this package is being evaluated.
    ///
    /// Bazel-compatible: available as a direct global in BUILD files.
    ///
    /// See: https://bazel.build/rules/lib/globals/build#module_name
    fn module_name(eval: &mut Evaluator) -> starlark::Result<NoneOr<String>> {
        let cell_name = BuildContext::from_context(eval)?
            .cell_info()
            .name()
            .to_string();
        Ok(NoneOr::Other(cell_name))
    }

    /// Returns the version of the Bazel module associated with the repository where
    /// this package is being evaluated.
    ///
    /// Bazel-compatible: available as a direct global in BUILD files.
    ///
    /// See: https://bazel.build/rules/lib/globals/build#module_version
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

    /// `get_cell_name()` can be called from either a `BUCK` file or a `.bzl` file,
    /// and returns the name of the cell where the `BUCK` file that started the call
    /// lives.
    ///
    /// For example, inside `foo//bar/baz/BUCK` the output will be `foo`.
    /// If that `BUCK` file does a `load("hello//world.bzl", "something")` then
    /// the result in that `.bzl` file will also be `foo`.
    fn get_cell_name(eval: &mut Evaluator) -> starlark::Result<String> {
        Ok(BuildContext::from_context(eval)?
            .cell_info()
            .name()
            .to_string())
    }

    /// Returns a list of the direct subpackages of the current package.
    ///
    /// Bazel built-in function that returns a sorted list of all subpackage paths
    /// (relative to the current BUILD file's package) that are immediate children.
    ///
    /// Example:
    /// ```python
    /// # In //foo/BUILD.bazel, with subpackages foo/bar and foo/baz:
    /// subpackages(include = ["**"])  # returns ["bar", "baz"]
    /// ```
    ///
    /// See: https://bazel.build/reference/be/functions#subpackages
    fn subpackages<'v>(
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        include: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        exclude: UnpackListOrTuple<String>,
        #[starlark(require = named, default = false)] allow_empty: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ValueOfUnchecked<'v, UnpackList<String>>> {
        let _ = allow_empty;
        let extra = ModuleInternals::from_context(eval, "subpackages")?;
        // Return direct subpackage paths relative to this package.
        // In Bazel, subpackages() returns strings like "bar", "baz" (just the last path component
        // or the package-relative path to the subpackage).
        let all_packages: Vec<String> = extra
            .sub_packages()
            .map(|p| p.as_str().to_owned())
            .collect();

        // Apply include/exclude glob filtering if patterns are provided
        let include_patterns = &include.items;
        let exclude_patterns = &exclude.items;

        let filtered: Vec<String> = if include_patterns.is_empty() {
            // No include patterns = return all (Bazel behavior)
            all_packages
        } else {
            let spec = GlobSpec::new(include_patterns, exclude_patterns)?;
            all_packages
                .into_iter()
                .filter(|pkg| spec.matches(pkg))
                .collect()
        };

        let res = filtered.iter().map(|s| s.as_str());
        Ok(eval.heap().alloc_typed_unchecked(AllocList(res)).cast())
    }

    /// Returns a dict of all rules instantiated so far in the current BUILD file.
    ///
    /// Bazel-compatible: available as a direct global in BUILD files.
    ///
    /// The keys are rule names, and the values are dicts containing rule attributes
    /// (name, kind, and all user-defined attributes serialized via JSON).
    ///
    /// See: https://bazel.build/rules/lib/globals/build#existing_rules
    fn existing_rules<'v>(eval: &mut Evaluator<'v, '_, '_>) -> starlark::Result<Value<'v>> {
        // When called outside a BUILD file context (e.g., from a module extension), return empty.
        let Ok(internals) = ModuleInternals::from_context(eval, "existing_rules") else {
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
                    if attr_name == "name" {
                        continue;
                    }
                    attrs_dict.insert(
                        attr_name,
                        crate::interpreter::natives::json_to_starlark_value(heap, &json_val),
                    );
                }
                let attrs_val = heap.alloc(AllocDict(attrs_dict));
                (name, attrs_val)
            })
            .collect();

        Ok(heap.alloc(AllocDict(result)))
    }

    /// Returns a dict of the attributes of the rule with the given name in the current BUILD file.
    /// Returns None if the rule doesn't exist.
    ///
    /// Bazel-compatible: available as a direct global in BUILD files.
    ///
    /// See: https://bazel.build/rules/lib/globals/build#existing_rule
    fn existing_rule<'v>(
        name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneOr<Value<'v>>> {
        let Ok(internals) = ModuleInternals::from_context(eval, "existing_rule") else {
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
            attrs_dict.insert(
                attr_name,
                crate::interpreter::natives::json_to_starlark_value(heap, &json_val),
            );
        }
        Ok(NoneOr::Other(heap.alloc(AllocDict(attrs_dict))))
    }

    /// Converts a label string to a Label object relative to the current package.
    ///
    /// Bazel-compatible: available as a direct global in BUILD files.
    ///
    /// For example, in package `//foo/bar`:
    /// - `package_relative_label(":target")` returns Label("//foo/bar:target")
    /// - `package_relative_label("//other:target")` returns Label("//other:target")
    ///
    /// See: https://bazel.build/rules/lib/globals/build#package_relative_label
    fn package_relative_label<'v>(
        label_string: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StringValue<'v>> {
        let build_ctx = BuildContext::from_context(eval)?;
        let base_path = build_ctx.base_path()?;
        let pkg = base_path.path();
        // If the label is already absolute (starts with //), return as-is
        let result = if label_string.starts_with("//") || label_string.starts_with("@") {
            label_string.to_owned()
        } else if let Some(name) = label_string.strip_prefix(':') {
            // ":name" -> "//pkg:name"
            format!("//{}:{}", pkg, name)
        } else {
            // "name" -> "//pkg:name"
            format!("//{}:{}", pkg, label_string)
        };
        Ok(eval.heap().alloc_str(&result))
    }
}
