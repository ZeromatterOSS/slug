# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for Bazel 8.0+ symbolic macros (macro() built-in)."""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_symbolic_macros_data")
async def test_symbolic_macro_basic(buck: Buck) -> None:
    """macro() defines a callable that invokes the implementation function."""
    result = await buck.build("//:basic_macro_test")
    output = result.get_build_report().output_for_target("//:basic_macro_test")
    content = output.read_text().strip()
    assert content == "macro_works", f"Expected 'macro_works', got '{content}'"


@buck_test(data_dir="test_symbolic_macros_data")
async def test_symbolic_macro_multi_target(buck: Buck) -> None:
    """macro() can create multiple targets via native.genrule/filegroup."""
    result = await buck.build("//:multi_test")
    output = result.get_build_report().output_for_target("//:multi_test")
    # filegroup wraps the genrule, so we should be able to build it
    assert output is not None


@buck_test(data_dir="test_symbolic_macros_data")
async def test_symbolic_macro_no_attrs(buck: Buck) -> None:
    """macro() works with no custom attrs - only name and visibility."""
    result = await buck.build("//:no_attrs_test")
    output = result.get_build_report().output_for_target("//:no_attrs_test")
    content = output.read_text().strip()
    assert content == "no_attrs", f"Expected 'no_attrs', got '{content}'"


@buck_test(data_dir="test_symbolic_macros_data")
async def test_symbolic_macro_default_attr(buck: Buck) -> None:
    """macro() uses default attribute values when not specified."""
    result = await buck.build("//:default_attr_test")
    output = result.get_build_report().output_for_target("//:default_attr_test")
    content = output.read_text().strip()
    assert content == "hello", f"Expected 'hello', got '{content}'"
