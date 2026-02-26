# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for module() directive in MODULE.bazel files."""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_module_directive_data")
async def test_module_directive_basic(buck: Buck) -> None:
    """Verify module() directive extracts name, version, compatibility_level."""
    # Test that basic module() directive is parsed correctly
    await buck.audit("cell")


@buck_test(data_dir="test_module_directive_data")
async def test_module_with_compatibility_level(buck: Buck) -> None:
    """Verify compatibility_level is parsed from module() directive."""
    await buck.audit("cell")
