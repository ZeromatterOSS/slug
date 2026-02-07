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
