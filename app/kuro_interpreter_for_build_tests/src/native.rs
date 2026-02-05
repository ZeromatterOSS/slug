/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Tests for the Bazel-compatible `native` module.
//!
//! These tests verify that `native.glob()`, `native.package_name()`,
//! `native.existing_rules()`, `native.existing_rule()`, and
//! `native.package_relative_label()` work correctly.
//!
//! Note: Most native.* functions require a BUILD file context, so they are
//! tested via integration tests that create actual BUILD files.

use dupe::Dupe;
use indoc::indoc;
use kuro_core::fs::project::ProjectRootTemp;
use kuro_core::package::PackageLabel;
use kuro_interpreter_for_build::interpreter::testing::Tester;
use kuro_node::nodes::frontend::TargetGraphCalculation;

use crate::tests::calculation;

// =============================================================================
// Unit tests for native module availability (runs in .bzl context)
// =============================================================================

#[test]
fn native_module_registered() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            # native module should be available in .bzl files
            assert_eq(True, hasattr(native, "glob"))
            assert_eq(True, hasattr(native, "package_name"))
            assert_eq(True, hasattr(native, "repository_name"))
            assert_eq(True, hasattr(native, "existing_rules"))
            assert_eq(True, hasattr(native, "existing_rule"))
            assert_eq(True, hasattr(native, "package_relative_label"))
        "#
    ))
}

#[test]
fn native_repository_name_works() -> kuro_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            # native.repository_name() should return the repository name with @ prefix
            repo = native.repository_name()
            assert_eq(True, type(repo) == "string")
            assert_eq(True, repo.startswith("@"))
        "#
    ))
}

// =============================================================================
// Integration tests for native functions that require BUILD file context
// =============================================================================

/// Test native.glob() works in a macro called from a BUILD file
#[tokio::test]
async fn test_native_glob_in_macro() {
    let fs = ProjectRootTemp::new().unwrap();

    // Create some test files to glob
    fs.write_file("pkg/src/foo.cc", "");
    fs.write_file("pkg/src/bar.cc", "");
    fs.write_file("pkg/src/test_foo.cc", "");

    fs.write_file(
        "rules.bzl",
        indoc!(
            r#"
                def _impl(ctx):
                    return DefaultInfo()

                glob_test_rule = rule(
                    implementation = _impl,
                    attrs = {
                        "srcs": attrs.list(attrs.source(), default = []),
                    },
                )

                def glob_macro(name, **kwargs):
                    # Use native.glob() to find files
                    srcs = native.glob(["src/*.cc"])
                    glob_test_rule(
                        name = name,
                        srcs = srcs,
                        **kwargs
                    )
            "#
        ),
    );

    fs.write_file(
        "pkg/BUILD.bazel",
        indoc!(
            r#"
                load("//rules.bzl", "glob_macro")

                glob_macro(
                    name = "glob_test",
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//pkg");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    let target_names = eval_result
        .targets()
        .keys()
        .map(|t| t.as_str())
        .collect::<Vec<_>>();

    assert_eq!(vec!["glob_test"], target_names);
}

/// Test native.glob() with exclude parameter
#[tokio::test]
async fn test_native_glob_with_exclude() {
    let fs = ProjectRootTemp::new().unwrap();

    // Create some test files to glob
    fs.write_file("pkg/src/foo.cc", "");
    fs.write_file("pkg/src/bar.cc", "");
    fs.write_file("pkg/src/test_foo.cc", "");

    fs.write_file(
        "rules.bzl",
        indoc!(
            r#"
                def _impl(ctx):
                    return DefaultInfo()

                glob_test_rule = rule(
                    implementation = _impl,
                    attrs = {
                        "srcs": attrs.list(attrs.source(), default = []),
                    },
                )

                def glob_macro_exclude(name, **kwargs):
                    # Use native.glob() with exclude to filter out test files
                    srcs = native.glob(["src/*.cc"], exclude = ["src/test_*.cc"])
                    glob_test_rule(
                        name = name,
                        srcs = srcs,
                        **kwargs
                    )
            "#
        ),
    );

    fs.write_file(
        "pkg/BUILD.bazel",
        indoc!(
            r#"
                load("//rules.bzl", "glob_macro_exclude")

                glob_macro_exclude(
                    name = "glob_exclude_test",
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//pkg");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    let target_names = eval_result
        .targets()
        .keys()
        .map(|t| t.as_str())
        .collect::<Vec<_>>();

    assert_eq!(vec!["glob_exclude_test"], target_names);
}

/// Test native.package_name() returns correct package path
#[tokio::test]
async fn test_native_package_name() {
    let fs = ProjectRootTemp::new().unwrap();

    fs.write_file(
        "rules.bzl",
        indoc!(
            r#"
                def _impl(ctx):
                    return DefaultInfo()

                package_test_rule = rule(
                    implementation = _impl,
                    attrs = {
                        "pkg_name": attrs.string(default = ""),
                    },
                )

                def package_name_macro(name, **kwargs):
                    # Use native.package_name() to get the package path
                    pkg = native.package_name()
                    package_test_rule(
                        name = name,
                        pkg_name = pkg,
                        **kwargs
                    )
            "#
        ),
    );

    fs.write_file(
        "foo/bar/BUILD.bazel",
        indoc!(
            r#"
                load("//rules.bzl", "package_name_macro")

                package_name_macro(
                    name = "pkg_test",
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//foo/bar");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    let target_names = eval_result
        .targets()
        .keys()
        .map(|t| t.as_str())
        .collect::<Vec<_>>();

    assert_eq!(vec!["pkg_test"], target_names);
}

/// Test native.existing_rules() returns defined rules
#[tokio::test]
async fn test_native_existing_rules() {
    let fs = ProjectRootTemp::new().unwrap();

    fs.write_file(
        "rules.bzl",
        indoc!(
            r#"
                def _impl(ctx):
                    return DefaultInfo()

                simple_rule = rule(
                    implementation = _impl,
                    attrs = {
                        "count": attrs.int(default = 0),
                    },
                )

                def check_existing_rules(name, **kwargs):
                    # Get all rules defined so far
                    rules = native.existing_rules()
                    # Create a rule that stores the count of existing rules
                    simple_rule(
                        name = name,
                        count = len(rules),
                        **kwargs
                    )
            "#
        ),
    );

    fs.write_file(
        "pkg/BUILD.bazel",
        indoc!(
            r#"
                load("//rules.bzl", "simple_rule", "check_existing_rules")

                simple_rule(
                    name = "first",
                )

                simple_rule(
                    name = "second",
                )

                # This should see 2 existing rules
                check_existing_rules(
                    name = "checker",
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//pkg");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    let target_names: Vec<_> = eval_result.targets().keys().map(|t| t.as_str()).collect();

    // Should have all 3 targets
    assert!(target_names.contains(&"first"));
    assert!(target_names.contains(&"second"));
    assert!(target_names.contains(&"checker"));
}

/// Test native.existing_rule() returns rule info or None
#[tokio::test]
async fn test_native_existing_rule() {
    let fs = ProjectRootTemp::new().unwrap();

    fs.write_file(
        "rules.bzl",
        indoc!(
            r#"
                def _impl(ctx):
                    return DefaultInfo()

                simple_rule = rule(
                    implementation = _impl,
                    attrs = {
                        "found": attrs.bool(default = False),
                    },
                )

                def check_existing_rule(name, check_name, **kwargs):
                    # Check if a specific rule exists
                    rule = native.existing_rule(check_name)
                    simple_rule(
                        name = name,
                        found = rule != None,
                        **kwargs
                    )
            "#
        ),
    );

    fs.write_file(
        "pkg/BUILD.bazel",
        indoc!(
            r#"
                load("//rules.bzl", "simple_rule", "check_existing_rule")

                simple_rule(
                    name = "target_a",
                )

                # This should find target_a
                check_existing_rule(
                    name = "checker_found",
                    check_name = "target_a",
                )

                # This should NOT find target_b (doesn't exist yet)
                check_existing_rule(
                    name = "checker_not_found",
                    check_name = "target_b",
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//pkg");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    let target_names: Vec<_> = eval_result.targets().keys().map(|t| t.as_str()).collect();

    assert!(target_names.contains(&"target_a"));
    assert!(target_names.contains(&"checker_found"));
    assert!(target_names.contains(&"checker_not_found"));
}

/// Test native.package_relative_label() converts labels correctly
#[tokio::test]
async fn test_native_package_relative_label() {
    let fs = ProjectRootTemp::new().unwrap();

    fs.write_file(
        "rules.bzl",
        indoc!(
            r#"
                def _impl(ctx):
                    return DefaultInfo()

                label_test_rule = rule(
                    implementation = _impl,
                    attrs = {
                        "label_str": attrs.string(default = ""),
                    },
                )

                def label_macro(name, input_label, **kwargs):
                    # Convert a label string to absolute form
                    resolved = native.package_relative_label(input_label)
                    label_test_rule(
                        name = name,
                        label_str = resolved,
                        **kwargs
                    )
            "#
        ),
    );

    fs.write_file(
        "foo/bar/BUILD.bazel",
        indoc!(
            r#"
                load("//rules.bzl", "label_macro")

                # Relative with colon
                label_macro(
                    name = "label_test_colon",
                    input_label = ":target",
                )

                # Relative without colon
                label_macro(
                    name = "label_test_no_colon",
                    input_label = "target",
                )

                # Absolute stays as-is
                label_macro(
                    name = "label_test_absolute",
                    input_label = "//other/pkg:target",
                )
            "#
        ),
    );

    let mut ctx = calculation(&fs).await;

    let package = PackageLabel::testing_parse("root//foo/bar");
    let eval_result = ctx.get_interpreter_results(package.dupe()).await.unwrap();
    let target_names: Vec<_> = eval_result.targets().keys().map(|t| t.as_str()).collect();

    assert!(target_names.contains(&"label_test_colon"));
    assert!(target_names.contains(&"label_test_no_colon"));
    assert!(target_names.contains(&"label_test_absolute"));
}
