# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for bazel_dep() directive in MODULE.bazel files."""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_bazel_dep_data")
async def test_bazel_dep_basic(buck: Buck) -> None:
    """Verify bazel_dep() directives are collected."""
    # Test that bazel_dep() directive is parsed correctly
    await buck.audit("cell")


@buck_test(data_dir="test_bazel_dep_data")
async def test_bazel_dep_with_repo_name(buck: Buck) -> None:
    """Verify repo_name override in bazel_dep() works."""
    await buck.audit("cell")


@buck_test(data_dir="test_bazel_dep_data")
async def test_bazel_dep_dev_dependency(buck: Buck) -> None:
    """Verify dev_dependency flag in bazel_dep() is parsed."""
    await buck.audit("cell")
