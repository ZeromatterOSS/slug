/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use dupe::Dupe;
use gazebo::prelude::SliceExt;
use indoc::indoc;
use kuro_build_api::interpreter::rule_defs::provider::registration::register_builtin_providers;
use kuro_common::legacy_configs::cells::BuckConfigBasedCells;
use kuro_common::legacy_configs::configs::LegacyBuckConfig;
use kuro_common::legacy_configs::configs::testing::TestConfigParserFileOps;
use kuro_common::package_listing::listing::PackageListing;
use kuro_common::package_listing::listing::testing::PackageListingExt;
use kuro_core::build_file_path::BuildFilePath;
use kuro_core::bzl::ImportPath;
use kuro_core::cells::cell_path::CellPath;
use kuro_core::cells::cell_path_with_allowed_relative_dir::CellPathWithAllowedRelativeDir;
use kuro_interpreter::file_loader::LoadedModules;
use kuro_interpreter::paths::module::OwnedStarlarkModulePath;
use kuro_interpreter::paths::path::StarlarkPath;
use kuro_interpreter_for_build::interpreter::testing::CellsData;
use kuro_interpreter_for_build::interpreter::testing::Tester;
use kuro_interpreter_for_build::interpreter::testing::run_simple_starlark_test;
use kuro_node::attrs::inspect_options::AttrInspectOptions;
use kuro_node::nodes::unconfigured::testing::targets_to_json;
use serde_json::json;

#[test]
fn test_eval_import() {
    let loaded = Tester::new()
        .unwrap()
        .add_import(
            &ImportPath::testing_new("root//some/package:defs.bzl"),
            indoc!(
                r#"
            one = 1
            hello = "world"
            "#
            ),
        )
        .unwrap();

    assert_eq!(1, loaded.env().get("one").unwrap().unpack_i32().unwrap());

    assert_eq!(
        "world",
        loaded.env().get("hello").unwrap().unpack_str().unwrap()
    );
}

#[test]
fn test_load() {
    let import_path = ImportPath::testing_new("root//imports:one.bzl");
    let mut tester = Tester::new().unwrap();
    tester
        .add_import(
            &import_path,
            indoc!(
                r#"
                    def concat(*args):
                      s = ""
                      for a in args:
                        s += a
                      return s
                    "#
            ),
        )
        .unwrap();

    let parse_result = tester
        .add_import(
            &ImportPath::testing_new("root//some/package:defs.bzl"),
            indoc!(
                r#"
                load("@root//imports:one.bzl", "concat")
                message = concat("hello", " ", "world!")
                "#
            ),
        )
        .unwrap();

    assert_eq!(
        "hello world!",
        parse_result
            .env()
            .get("message")
            .unwrap()
            .unpack_str()
            .unwrap()
    );
}

#[test]
fn test_eval_build_file() {
    let mut tester = Tester::new().unwrap();
    tester.additional_globals(register_builtin_providers);

    tester
        .add_import(
            &ImportPath::testing_new("root//:rules.bzl"),
            indoc!(
                r#"
                def _impl(ctx):
                    return DefaultInfo()

                export_file = rule(
                    impl = _impl,
                    attrs = {
                        "src": attrs.any(),
                    },
                )

                java_library = rule(
                    impl = _impl,
                    attrs = {
                        "srcs": attrs.list(attrs.any()),
                    },
                )
            "#
            ),
        )
        .unwrap();

    tester
        .add_import(
            &ImportPath::testing_new_cross_cell("root", "imports", "one.bzl", "root"),
            indoc!(
                r#"
                    load("@root//:rules.bzl", "export_file")

                    def some_macro(name, **kwargs):
                        export_file(
                            name=name+"-exported",
                            **kwargs
                        )
                    "#
            ),
        )
        .unwrap();

    let build_path = BuildFilePath::testing_new("root//some/package:BUILD");
    let eval_result = tester
        .eval_build_file(
            &build_path,
            indoc!(
                r#"
                load("@root//imports:one.bzl", "some_macro")
                load("@root//:rules.bzl", "java_library")

                some_macro(
                    name = "invoke_some",
                    src = "some.file",
                )
                java_library(
                    name = "java",
                    srcs = glob(["**/*.java"]),
                )
                "#
            ),
            PackageListing::testing_files(&["file1.java", "file2.java"]),
        )
        .unwrap();

    assert_eq!(build_path.package(), eval_result.package());
    let target_names = eval_result
        .targets()
        .keys()
        .map(|t| t.as_str().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(vec!["invoke_some-exported", "java"], target_names);
}

fn cells() -> CellsData {
    let BuckConfigBasedCells { cell_resolver, .. } =
        futures::executor::block_on(BuckConfigBasedCells::testing_parse_with_file_ops(
            &mut TestConfigParserFileOps::new(&[(
                ".buckconfig",
                indoc!(
                    r#"
                    [cells]
                        root = .
                        cell1 = project/cell1
                        cell2 = project/cell2
                        xalias2 = project/cell2
                    "#
                ),
            )])
            .unwrap(),
            &[],
        ))
        .unwrap();
    (
        cell_resolver.root_cell_cell_alias_resolver().dupe(),
        cell_resolver,
        LegacyBuckConfig::empty(),
        CellPathWithAllowedRelativeDir::new(
            CellPath::testing_new("cell1//config/foo"),
            Some(CellPath::testing_new("cell1//config")),
        ),
    )
}

#[test]
fn test_find_imports() {
    let tester = Tester::with_cells(cells()).unwrap();
    let path = BuildFilePath::testing_new("cell1//config/foo:BUCK");
    let parse_result = tester.parse(
        StarlarkPath::BuildFile(&path),
        indoc!(
            r#"
            a = 1
        "#
        ),
    );

    assert!(parse_result.imports().is_empty());

    let parse_result = tester.parse(
        StarlarkPath::BuildFile(&path),
        indoc!(
            r#"
            # some documentation
            """ and a string """

            load("//imports:one.bzl", "some_macro")
            load("@cell1//:one.bzl", "some_macro")
            load("@xalias2//:two.bzl", "some_macro")

            # some other comments
            load(":other.bzl", "some_macro")
            load("../bar/three.bzl", "some_macro")
        "#
        ),
    );

    assert_eq!(
        &[
            "root//imports/one.bzl@cell1",
            "cell1//one.bzl",
            "cell2//two.bzl@cell1",
            "cell1//config/foo/other.bzl",
            "cell1//config/bar/three.bzl",
        ],
        parse_result.imports().map(|e| e.1.to_string()).as_slice()
    );
}

#[test]
fn test_root_import() {
    let mut tester = Tester::with_cells(
        kuro_interpreter_for_build::interpreter::testing::cells(Some(indoc!(
            r#"
            [buildfile]
                includes = //include.bzl
        "#
        )))
        .unwrap(),
    )
    .unwrap();

    tester.additional_globals(register_builtin_providers);

    let import_path = ImportPath::testing_new("root//:include.bzl");
    tester
        .add_import(
            &import_path,
            indoc!(
                r#"
                    some_var = 1
                    def some_func():
                       return "hello"

                    def _impl(ctx):
                        return DefaultInfo()

                    export_file = rule(
                        impl = _impl,
                        attrs = {
                            "level": attrs.int(),
                        },
                    )
        "#
            ),
        )
        .unwrap();

    let build_path = BuildFilePath::testing_new("root//some/package:BUCK");
    let eval_result = tester
        .eval_build_file(
            &build_path,
            indoc!(
                r#"
                export_file(
                    name = some_func(),
                    level = some_var,
                )
                "#
            ),
            PackageListing::testing_files(&["file1.java", "file2.java"]),
        )
        .unwrap();

    assert_eq!(build_path.package(), eval_result.package());
    let target_names = eval_result
        .targets()
        .keys()
        .map(|t| t.as_str().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(vec!["hello"], target_names);
}

#[test]
fn test_package_import() -> kuro_error::Result<()> {
    let mut tester = Tester::with_cells(kuro_interpreter_for_build::interpreter::testing::cells(
        Some(indoc!(
            r#"
            [buildfile]
                package_includes = src=>//include.bzl::func_alias=some_func
        "#
        )),
    )?)?;

    let import_path = ImportPath::testing_new("root//:include.bzl");
    tester.add_import(
        &import_path,
        indoc!(
            r#"
            def _impl(ctx):
                pass
            export_file = rule(impl=_impl, attrs = {})

            def some_func(name):
                export_file(name = name)
        "#
        ),
    )?;

    let build_path = BuildFilePath::testing_new("root//src/package:BUCK");
    let eval_result = tester.eval_build_file(
        &build_path,
        indoc!(
            r#"
                implicit_package_symbol("func_alias")(
                    implicit_package_symbol("missing", "DEFAULT")
                )
                "#
        ),
        PackageListing::testing_files(&["file1.java", "file2.java"]),
    )?;
    assert_eq!(build_path.package(), eval_result.package());
    assert_eq!(
        json!({
                "DEFAULT": {
                    "__type__": "root//include.bzl:export_file",
                    "applicable_licenses": [],
                    "compatible_with": [],
                    "default_target_platform": null,
                    "deprecation": null,
                    "exec_compatible_with": [],
                    "features": [],
                    "name": "DEFAULT",
                    "tags": [],
                    "target_compatible_with": [],
                    "testonly": false,
                    "modifiers": [],
                    "tests": [],
                    "visibility": [],
                    "within_view": ["PUBLIC"],
                    "metadata": {},
                },
        }),
        targets_to_json(
            eval_result.targets(),
            build_path.package(),
            AttrInspectOptions::All
        )?
    );
    Ok(())
}

#[test]
fn eval() -> kuro_error::Result<()> {
    let mut tester = Tester::new()?;
    let content = indoc!(
        r#"
            def _impl(ctx):
                pass
            export_file = rule(impl=_impl, attrs = {})

            def test():
                assert_eq("some/package", package_name())
                assert_eq("@", repository_name())
                assert_eq(package_name(), get_base_path())

                export_file(name = "rule_name")
                assert_eq(True, rule_exists("rule_name"))
                assert_eq(False, rule_exists("not_rule_name"))

                print("some message")
                print("multiple", "strings")
            "#
    );
    tester.run_starlark_test(content)?;
    Ok(())
}

#[test]
fn test_builtins() -> kuro_error::Result<()> {
    // Public natives like `json.encode` live at the top level.
    run_simple_starlark_test(indoc!(
        r#"
            def test():
                assert_eq(json.encode({}), "{}")
            "#
    ))?;

    // Internals are reached via the `__internal__` namespace and must NOT
    // appear at the top level. `kuro_fail` is registered in
    // `register_all_internals` (kuro_interpreter_for_build::interpreter::functions::internals).
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_test_expecting_error(
        indoc!(
            r#"
            def test():
                kuro_fail("message")
            "#
        ),
        "Variable `kuro_fail` not found",
    );
    Ok(())
}
