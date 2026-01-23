# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for multi-module projects using local_path_override()."""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_multiple_local_modules_recognized(buck: Buck) -> None:
    """Verify that multiple local_path_override() calls are parsed correctly."""
    result = await buck.audit_cell()

    assert result.returncode == 0

    # Check that all cells are recognized
    output = result.stdout
    assert "root" in output
    assert "lib_a" in output
    assert "lib_b" in output


@buck_test()
async def test_each_local_module_has_own_module_bazel(buck: Buck) -> None:
    """Verify that each local module's MODULE.bazel is found."""
    # Query cells to verify both local modules are visible
    result = await buck.audit_cell()

    assert result.returncode == 0

    # Both local modules should be in the output
    assert "lib_a" in result.stdout
    assert "lib_b" in result.stdout


@buck_test()
async def test_targets_in_both_local_modules(buck: Buck) -> None:
    """Verify that targets can be queried from both local modules."""
    # Query lib_a
    result_a = await buck.targets("lib_a//...")
    assert result_a.returncode == 0

    # Query lib_b
    result_b = await buck.targets("lib_b//...")
    assert result_b.returncode == 0
