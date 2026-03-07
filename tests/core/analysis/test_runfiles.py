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


@buck_test(data_dir="test_runfiles_data")
async def test_runfiles_collect(buck: Buck) -> None:
    result = await buck.build("//:collector")
    output = result.get_build_report().output_for_target("//:collector")

    content = output.read_text().strip().splitlines()
    assert content == ["data.txt", "dep.txt", "own.txt", "runtime.txt"]


@buck_test(data_dir="test_runfiles_data")
async def test_runfiles_merge(buck: Buck) -> None:
    """runfiles.merge() combines two runfiles objects."""
    result = await buck.build("//:runfiles_merge")
    output = result.get_build_report().output_for_target("//:runfiles_merge")

    content = sorted(output.read_text().strip().splitlines())
    assert "dep.txt" in content
    assert "data.txt" in content


@buck_test(data_dir="test_runfiles_data")
async def test_runfiles_merge_all(buck: Buck) -> None:
    """runfiles.merge_all() combines a list of runfiles objects."""
    result = await buck.build("//:runfiles_merge_all")
    output = result.get_build_report().output_for_target("//:runfiles_merge_all")

    content = sorted(output.read_text().strip().splitlines())
    assert "dep.txt" in content
    assert "data.txt" in content
    assert "runtime.txt" in content


@buck_test(data_dir="test_runfiles_data")
async def test_runfiles_transitive_files(buck: Buck) -> None:
    """ctx.runfiles(transitive_files=depset([...])) includes files from the depset."""
    result = await buck.build("//:runfiles_transitive")
    output = result.get_build_report().output_for_target("//:runfiles_transitive")

    content = sorted(output.read_text().strip().splitlines())
    assert "dep.txt" in content
    assert "data.txt" in content


@buck_test(data_dir="test_runfiles_data")
async def test_runfiles_files_depset(buck: Buck) -> None:
    """runfiles.files returns a depset of all files."""
    result = await buck.build("//:runfiles_files_attr")
    output = result.get_build_report().output_for_target("//:runfiles_files_attr")

    content = output.read_text().strip()
    # runfiles.files is a depset; to_list() should give us the files
    assert "dep.txt" in content
