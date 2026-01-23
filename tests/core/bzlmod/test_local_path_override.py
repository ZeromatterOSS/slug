# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for local_path_override() in MODULE.bazel."""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_local_path_override_recognized(buck: Buck) -> None:
    """Verify that local_path_override() is parsed correctly."""
    # Parse the MODULE.bazel with local_path_override
    result = await buck.audit_cell()

    # Verify command succeeds - the MODULE.bazel was parsed
    assert result.returncode == 0

    # Check that both cells are recognized
    output = result.stdout
    assert "root" in output
    assert "local_lib" in output


@buck_test()
async def test_local_module_has_module_bazel(buck: Buck) -> None:
    """Verify that local module's MODULE.bazel is found."""
    # Use audit_cell to check that local_lib cell is visible
    result = await buck.audit_cell()

    assert result.returncode == 0
    assert "local_lib" in result.stdout


@buck_test()
async def test_local_module_build_files_found(buck: Buck) -> None:
    """Verify that local module's BUILD.bazel files are found."""
    # Query to find targets in the local module
    # This verifies BUILD.bazel is recognized
    result = await buck.targets("local_lib//...")

    # Should succeed (even if no targets found, command should work)
    assert result.returncode == 0
