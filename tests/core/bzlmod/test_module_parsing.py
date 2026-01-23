# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for MODULE.bazel file parsing."""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.asserts import expect_failure
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_module_bazel_recognized(buck: Buck) -> None:
    """Verify that MODULE.bazel is recognized as workspace root marker."""
    # The workspace should be recognized by MODULE.bazel presence
    result = await buck.audit_cell()
    # Just verify the command succeeds - MODULE.bazel was found
    assert result.returncode == 0


@buck_test()
async def test_module_bazel_syntax_error(buck: Buck) -> None:
    """Verify that invalid MODULE.bazel syntax gives helpful error."""
    # This test uses a data directory with invalid MODULE.bazel syntax
    result = await expect_failure(buck.audit_cell())
    assert "MODULE.bazel" in result.stderr or "syntax" in result.stderr.lower()
