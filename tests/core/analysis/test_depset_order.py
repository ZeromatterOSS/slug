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
        "//:default",
        "//:default_infer",
    )

    assert _read_lines(buck, "//:preorder", result) == ["c", "a", "b"]
    assert _read_lines(buck, "//:postorder", result) == ["a", "b", "c"]

    topological = _read_lines(buck, "//:topological", result)
    assert set(topological) == {"a", "b", "c"}

    default = _read_lines(buck, "//:default", result)
    assert set(default) == {"a", "b", "c"}

    assert _read_lines(buck, "//:default_infer", result) == ["c", "a", "b"]


@buck_test(data_dir="test_depset_order_data")
async def test_depset_order_mismatch(buck: Buck) -> None:
    await expect_failure(
        buck.build("//:mismatch"),
        stderr_regex="transitive elements must all have the same order",
    )


@buck_test(data_dir="test_depset_order_data")
async def test_depset_cross_rule_traversal(buck: Buck) -> None:
    """Depsets passed through providers can be traversed in consumer rules.

    This tests the critical fix for FrozenLiveDepset.direct/transitive
    attributes which allows depset.to_list() to work on frozen depsets
    received via providers from other rules.
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
