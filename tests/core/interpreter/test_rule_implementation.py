# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for rule() function implementation parameter.

Verifies that both Bazel-style (`implementation`) and Kuro-style (`impl`)
parameter names work correctly for defining rule implementation functions.
"""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.asserts import expect_failure
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_rule_with_implementation_parameter(buck: Buck) -> None:
    """Test that rule() works with Bazel-style `implementation` parameter."""
    output = await buck.build("//:bazel_style")
    assert output.returncode == 0


@buck_test()
async def test_rule_with_impl_parameter(buck: Buck) -> None:
    """Test that rule() works with Kuro-style `impl` parameter."""
    output = await buck.build("//:kuro_style")
    assert output.returncode == 0


@buck_test()
async def test_rule_with_both_impl_and_implementation_fails(buck: Buck) -> None:
    """Test that rule() fails when both `impl` and `implementation` are specified."""
    await expect_failure(
        buck.uquery("//:both_params", "--console=none"),
        stderr_regex="Cannot specify both `impl` and `implementation` in rule",
    )


@buck_test()
async def test_rule_without_implementation_fails(buck: Buck) -> None:
    """Test that rule() fails when neither `impl` nor `implementation` is specified."""
    await expect_failure(
        buck.uquery("//:no_impl", "--console=none"),
        stderr_regex="Missing `implementation` function in rule",
    )
