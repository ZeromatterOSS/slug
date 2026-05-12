/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the root directory of this source tree. You
 * may select, at your option, one of the above-listed licenses.
 */

use indoc::indoc;
use slug_build_api::interpreter::rule_defs::depset::register_depset;
use slug_interpreter_for_build::interpreter::testing::Tester;

fn depset_tester() -> Tester {
    let mut tester = Tester::new().unwrap();
    tester.additional_globals(register_depset);
    tester
}

#[test]
fn depset_public_surface_matches_bazel_9_1_0() -> slug_error::Result<()> {
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
fn depset_public_surface_rejects_slug_prototype_extensions() {
    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                len(depset(["x"]))
            "#
        ),
        "in call to len(), parameter 'x' got value of type 'depset', want 'iterable or string'",
    );

    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                depset(["x"]) | depset(["y"])
            "#
        ),
        "unsupported binary operation: depset | depset",
    );
}

#[test]
fn depset_orders_match_bazel_9_1_0_probe() -> slug_error::Result<()> {
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
fn depset_to_list_dedupes_hashable_values_preserving_order() -> slug_error::Result<()> {
    let mut tester = depset_tester();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        frozen_dedup = depset([("z", 3), ("z", 3)])

        def test():
            assert_eq(
                [("x", 1), ("y", 2)],
                depset([("x", 1), ("y", 2), ("x", 1), ("y", 2)]).to_list(),
            )
            assert_eq(
                [("y", 2), ("x", 1)],
                depset([("x", 1)], transitive = [depset([("y", 2), ("x", 1), ("y", 2)])]).to_list(),
            )
            assert_eq([("z", 3)], frozen_dedup.to_list())
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
                depset(["x"], order = "stable")
            "#
        ),
        "Invalid order: stable",
    );

    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                depset(transitive = ["x"])
            "#
        ),
        "at index 0 of transitive, got element of type string, want depset",
    );

    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                depset(["x", 1])
            "#
        ),
        "cannot add an item of type 'int' to a depset of 'string'",
    );

    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                depset([1], transitive = [depset(["x"])])
            "#
        ),
        "cannot add an item of type 'string' to a depset of 'int'",
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
        "Order 'preorder' is incompatible with order 'postorder'",
    );
}

#[test]
fn depset_depth_validation_matches_bazel_9_1_0_probe() -> slug_error::Result<()> {
    let mut tester = depset_tester();
    tester.run_starlark_bzl_test(indoc!(
        r#"
        def test():
            d = depset(["0"])
            for i in range(3499):
                d = depset([str(i + 1)], transitive = [d])
            assert_eq(["0"], d.to_list()[:1])

            # Bazel does not increase depset depth for a parent with only
            # transitive children.
            for _ in range(3501):
                d = depset(transitive = [d])
            assert_eq(["0"], d.to_list()[:1])
        "#
    ))?;

    let mut tester = depset_tester();
    tester.run_starlark_bzl_test_expecting_error(
        indoc!(
            r#"
            def test():
                d = depset(["0"])
                for i in range(3500):
                    d = depset([str(i + 1)], transitive = [d])
            "#
        ),
        "depset depth 3501 exceeds limit (3500)",
    );

    Ok(())
}
