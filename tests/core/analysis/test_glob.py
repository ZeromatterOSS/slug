# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_glob_data")
async def test_glob_simple(buck: Buck) -> None:
    """glob(['*.txt']) matches all .txt files in the current directory."""
    result = await buck.build("//:glob_all_txt")
    output = result.get_build_report().output_for_target("//:glob_all_txt")
    names = set(output.read_text().strip().splitlines())
    assert "a.txt" in names
    assert "b.txt" in names
    assert "excluded.txt" in names
    # Should not include BUILD.bazel or defs.bzl
    assert "BUILD.bazel" not in names
    assert "defs.bzl" not in names


@buck_test(data_dir="test_glob_data")
async def test_glob_exclude(buck: Buck) -> None:
    """glob(['*.txt'], exclude=['excluded.txt']) skips the excluded file."""
    result = await buck.build("//:glob_exclude")
    output = result.get_build_report().output_for_target("//:glob_exclude")
    names = set(output.read_text().strip().splitlines())
    assert "a.txt" in names
    assert "b.txt" in names
    assert "excluded.txt" not in names


@buck_test(data_dir="test_glob_data")
async def test_glob_recursive(buck: Buck) -> None:
    """glob(['**/*.txt']) recursively matches .txt files in subdirectories."""
    result = await buck.build("//:glob_recursive")
    output = result.get_build_report().output_for_target("//:glob_recursive")
    names = set(output.read_text().strip().splitlines())
    # Should include root-level .txt files
    assert "a.txt" in names
    assert "b.txt" in names
    # Should include nested files (basename is just the filename)
    assert "nested.txt" in names


@buck_test(data_dir="test_glob_data")
async def test_glob_multiple_patterns(buck: Buck) -> None:
    """glob() accepts multiple patterns and merges matches."""
    result = await buck.build("//:glob_multi_pattern")
    output = result.get_build_report().output_for_target("//:glob_multi_pattern")
    names = set(output.read_text().strip().splitlines())
    # .txt files should be included
    assert "a.txt" in names
    assert "b.txt" in names
    # .bzl files should also be included
    assert "defs.bzl" in names
