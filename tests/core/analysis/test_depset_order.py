# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.asserts import expect_failure
from buck2.tests.e2e_util.buck_workspace import buck_test


def _read_lines(buck: Buck, target: str, result) -> list[str]:
    output = result.get_build_report().output_for_target(target)
    return output.read_text().strip().splitlines()


@buck_test(data_dir="test_depset_order_data")
async def test_depset_orders(buck: Buck) -> None:
    result = await buck.build(
        "//:preorder",
        "//:postorder",
        "//:topological",
        "//:topological_diamond",
        "//:default",
        "//:default_infer",
    )

    assert _read_lines(buck, "//:preorder", result) == ["c", "a", "b"]
    assert _read_lines(buck, "//:postorder", result) == ["a", "b", "c"]
    assert _read_lines(buck, "//:topological", result) == ["c", "a", "b"]
    assert _read_lines(buck, "//:topological_diamond", result) == ["d", "b", "c", "a"]
    assert _read_lines(buck, "//:default", result) == ["a", "b", "c"]
    assert _read_lines(buck, "//:default_infer", result) == ["a", "b", "c"]


@buck_test(data_dir="test_depset_order_data")
async def test_depset_order_mismatch(buck: Buck) -> None:
    await expect_failure(
        buck.build("//:mismatch"),
        stderr_regex="incompatible",
    )


@buck_test(data_dir="test_depset_order_data")
async def test_depset_cross_rule_traversal(buck: Buck) -> None:
    """Depsets passed through providers can be traversed in consumer rules.

    This verifies that frozen depsets keep their internal graph shape when
    passed through providers, without exposing Starlark .direct/.transitive.
    """
    result = await buck.build("//:depset_consumer")
    output = result.get_build_report().output_for_target("//:depset_consumer")

    content = output.read_text().strip().splitlines()
    # All items from the transitive depset chain should be present
    assert set(content) == {"item_a1", "item_a2", "item_b1", "item_c1"}


@buck_test(data_dir="test_depset_order_data")
async def test_depset_keyword_direct_transitive(buck: Buck) -> None:
    """depset(direct=[...], transitive=[...]) keyword form works correctly."""
    result = await buck.build("//:depset_keyword")
    output = result.get_build_report().output_for_target("//:depset_keyword")
    content = set(output.read_text().strip().splitlines())
    assert content == {"x", "y", "z"}, f"Expected {{x, y, z}}, got {content}"


@buck_test(data_dir="test_depset_order_data")
async def test_depset_union_operator(buck: Buck) -> None:
    """Bazel 9 depset does not support the Slug prototype | operator."""
    await expect_failure(
        buck.build("//:depset_union"),
        stderr_regex="unsupported binary operation",
    )


@buck_test(data_dir="test_depset_order_data")
async def test_depset_order_attribute(buck: Buck) -> None:
    """Bazel 9 depset does not expose a public .order attribute."""
    await expect_failure(
        buck.build("//:depset_order_attr"),
        stderr_regex="order",
    )


@buck_test(data_dir="test_depset_order_data")
async def test_depset_len(buck: Buck) -> None:
    """Bazel 9 depset is not iterable and does not implement len()."""
    await expect_failure(
        buck.build("//:depset_len"),
        stderr_regex="iterable or string",
    )


@buck_test(data_dir="test_depset_order_data")
async def test_depset_transitive_set_bridge(buck: Buck) -> None:
    """The explicit Slug bridge is lossy but deterministic for basic values."""
    result = await buck.build("//:depset_bridge")
    assert _read_lines(buck, "//:depset_bridge", result) == [
        "nodes=2,2",
        "default=a,b,c,d",
        "preorder=c,d,a,b",
    ]
