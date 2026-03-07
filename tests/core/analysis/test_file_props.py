# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

import sys

import pytest
from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_file_props_data")
async def test_source_file_basename(buck: Buck) -> None:
    """File.basename returns the filename without directory."""
    result = await buck.build("//:source_file_props")
    output = result.get_build_report().output_for_target("//:source_file_props")
    props = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert props["src.basename"] == "source.txt", f"Got: {props['src.basename']!r}"


@buck_test(data_dir="test_file_props_data")
async def test_source_file_extension(buck: Buck) -> None:
    """File.extension returns the file extension without dot."""
    result = await buck.build("//:source_file_props")
    output = result.get_build_report().output_for_target("//:source_file_props")
    props = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert props["src.extension"] == "txt", f"Got: {props['src.extension']!r}"


@buck_test(data_dir="test_file_props_data")
async def test_source_file_is_source(buck: Buck) -> None:
    """File.is_source is True for source files, False for generated files."""
    result = await buck.build("//:source_file_props")
    output = result.get_build_report().output_for_target("//:source_file_props")
    props = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert props["src.is_source"] == "True", f"Source file should have is_source=True, got: {props['src.is_source']!r}"


@buck_test(data_dir="test_file_props_data")
async def test_source_file_not_directory(buck: Buck) -> None:
    """File.is_directory is False for regular files."""
    result = await buck.build("//:source_file_props")
    output = result.get_build_report().output_for_target("//:source_file_props")
    props = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert props["src.is_directory"] == "False", f"Regular file should have is_directory=False"


@buck_test(data_dir="test_file_props_data")
async def test_source_file_short_path(buck: Buck) -> None:
    """File.short_path returns the package-relative path for source files."""
    result = await buck.build("//:source_file_props")
    output = result.get_build_report().output_for_target("//:source_file_props")
    props = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    # For root package source file: short_path = "source.txt" (no directory prefix)
    assert props["src.short_path"] == "source.txt", f"Got: {props['src.short_path']!r}"


@buck_test(data_dir="test_file_props_data")
async def test_source_file_path_ends_with_basename(buck: Buck) -> None:
    """File.path ends with File.basename."""
    result = await buck.build("//:source_file_props")
    output = result.get_build_report().output_for_target("//:source_file_props")
    props = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert props["src.path.endswith_basename"] == "True"


@buck_test(data_dir="test_file_props_data")
async def test_generated_file_is_not_source(buck: Buck) -> None:
    """File.is_source is False for generated files."""
    result = await buck.build("//:generated_file_props")
    output = result.get_build_report().output_for_target("//:generated_file_props")
    props = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert props["gen.is_source"] == "False", f"Generated file should have is_source=False"


@buck_test(data_dir="test_file_props_data")
async def test_generated_file_root_contains_buck_out(buck: Buck) -> None:
    """Generated File.root.path contains buck-out."""
    result = await buck.build("//:generated_file_props")
    output = result.get_build_report().output_for_target("//:generated_file_props")
    props = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert props["gen.root.path.contains_buck_out"] == "True", f"root.path should contain buck-out"


@buck_test(data_dir="test_file_props_data")
async def test_generated_file_short_path_no_buck_out(buck: Buck) -> None:
    """Generated File.short_path does NOT contain buck-out prefix."""
    result = await buck.build("//:generated_file_props")
    output = result.get_build_report().output_for_target("//:generated_file_props")
    props = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert props["gen.short_path.no_buck_out"] == "True", f"short_path should not contain buck-out"


@buck_test(data_dir="test_file_props_data")
async def test_generated_file_path_equals_root_plus_short(buck: Buck) -> None:
    """Generated File.path == File.root.path + '/' + File.short_path."""
    result = await buck.build("//:generated_file_props")
    output = result.get_build_report().output_for_target("//:generated_file_props")
    props = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert props["gen.path.equals_root_plus_short"] == "True", \
        f"path should equal root.path + '/' + short_path"


@buck_test(data_dir="test_file_props_data")
@pytest.mark.skipif(sys.platform == "win32", reason="run_shell mkdir not portable on Windows")
async def test_directory_artifact_is_directory(buck: Buck) -> None:
    """Directory artifacts have is_directory=True."""
    result = await buck.build("//:dir_artifact_props")
    output = result.get_build_report().output_for_target("//:dir_artifact_props")
    props = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert props["dir.is_directory"] == "True", f"Directory artifact should have is_directory=True"
    assert props["dir.is_source"] == "False", f"Directory artifact should not be source"
