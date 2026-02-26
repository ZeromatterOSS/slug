# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for MODULE.bazel file parsing."""

import pytest
from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_module_parsing_data")
async def test_module_bazel_recognized(buck: Buck) -> None:
    """Verify that MODULE.bazel is recognized as workspace root marker."""
    # The workspace should be recognized by MODULE.bazel presence
    await buck.audit("cell")


@pytest.mark.skip(
    reason="Requires a separate data directory with invalid MODULE.bazel syntax. "
    "Not yet set up for kuro test infrastructure."
)
@buck_test(data_dir="test_module_parsing_data")
async def test_module_bazel_syntax_error(buck: Buck) -> None:
    """Verify that invalid MODULE.bazel syntax gives helpful error."""
    pass
