/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the root directory of this source tree. You
 * may select, at your option, one of the above-listed licenses.
 */

use indoc::indoc;
use kuro_build_api::interpreter::rule_defs::depset::register_depset;
use kuro_interpreter_for_build::interpreter::testing::Tester;

fn depset_tester() -> Tester {
    let mut tester = Tester::new().unwrap();
    tester.additional_globals(register_depset);
    tester
}

#[test]
fn depset_public_surface_matches_bazel_9_1_0() -> kuro_error::Result<()> {
    let mut tester = depset_tester();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            empty = depset()
            non_empty = depset(["x"])

            assert_eq("depset", type(empty))
            assert_eq([], empty.to_list())
            assert_false(bool(empty))
            assert_true(bool(non_empty))
            assert_false(hasattr(non_empty, "order"))
            assert_false(hasattr(non_empty, "direct"))
            assert_false(hasattr(non_empty, "transitive"))
            assert_eq([], depset(transitive = None).to_list())
            assert_eq([], depset(direct = None).to_list())
        "#
    ))
}

#[test]
fn depset_public_surface_rejects_kuro_prototype_extensions() {
    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                len(depset(["x"]))
            "#
        ),
        "want 'iterable or string'",
    );

    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                depset(["x"]) | depset(["y"])
            "#
        ),
        "unsupported binary operation",
    );
}

#[test]
fn depset_orders_match_bazel_9_1_0_probe() -> kuro_error::Result<()> {
    let mut tester = depset_tester();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen_leaf = depset(["frozen_leaf"], order = "preorder")
        frozen_parent = depset(["frozen_parent"], transitive = [frozen_leaf])

        def test():
            pre_a = depset(["a"], order = "preorder")
            pre_b = depset(["b"], order = "preorder")
            pre_c = depset(["c"], transitive = [pre_a, pre_b], order = "preorder")
            assert_eq(["c", "a", "b"], pre_c.to_list())

            post_a = depset(["a"], order = "postorder")
            post_b = depset(["b"], order = "postorder")
            post_c = depset(["c"], transitive = [post_a, post_b], order = "postorder")
            assert_eq(["a", "b", "c"], post_c.to_list())

            default_a = depset(["a"], order = "preorder")
            default_c = depset(["c"], transitive = [default_a])
            assert_eq(["a", "c"], default_c.to_list())

            mixed_default = depset(
                ["c"],
                transitive = [
                    depset(["a"], order = "preorder"),
                    depset(["b"], order = "postorder"),
                ],
            )
            assert_eq(["a", "b", "c"], mixed_default.to_list())

            top_a = depset(["a"], order = "topological")
            top_b = depset(["b"], transitive = [top_a], order = "topological")
            top_c = depset(["c"], transitive = [top_a], order = "topological")
            top_d = depset(["d"], transitive = [top_b, top_c], order = "topological")
            assert_eq(["d", "b", "c", "a"], top_d.to_list())

            assert_eq(["a", "b"], depset(["a", "a"], transitive = [depset(["a", "b"])]).to_list())
            assert_eq(["frozen_leaf", "frozen_parent", "live"], depset(["live"], transitive = [frozen_parent]).to_list())
        "#
    ))
}

#[test]
fn depset_validation_matches_bazel_9_1_0_probe() {
    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                depset(transitive = ["x"])
            "#
        ),
        "transitive elements must be depsets",
    );

    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                depset(["x", 1])
            "#
        ),
        "cannot add an item of type",
    );

    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                depset([1], transitive = [depset(["x"])])
            "#
        ),
        "cannot add an item of type",
    );

    let mut tester = depset_tester();
    tester
        .run_starlark_bzl_test(indoc!(
            r#"
        def test():
            assert_eq([1], depset([1], transitive = [depset()]).to_list())
        "#
        ))
        .unwrap();

    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                depset([["x"]])
            "#
        ),
        "depset elements must not be mutable values",
    );

    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                depset([{"x": "y"}])
            "#
        ),
        "depset elements must not be mutable values",
    );

    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                depset(["x"], order = "preorder", transitive = [depset(["y"], order = "postorder")])
            "#
        ),
        "incompatible",
    );
}
