# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""Tests for cc_common.create_compilation_context() and related context APIs."""

import pytest

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_create_compilation_context_data")
async def test_basic_compilation_context(buck: Buck) -> None:
    """cc_common.create_compilation_context() creates context with all field types."""
    result = await buck.build("//:basic_context")
    output = result.get_build_report().output_for_target("//:basic_context")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["type"] == "CcCompilationContext"
    # Includes
    assert int(lines["includes_count"]) == 2
    assert "include/" in lines["includes"]
    assert "src/" in lines["includes"]
    # Defines
    assert int(lines["defines_count"]) >= 2
    assert "VERSION" in lines["defines"]
    assert "DEBUG" in lines["defines"]
    # Quote includes
    assert int(lines["quote_includes_count"]) == 1
    assert "." in lines["quote_includes"]
    # System includes
    assert int(lines["system_includes_count"]) == 1
    assert "/usr/include" in lines["system_includes"]


@buck_test(data_dir="test_create_compilation_context_data")
async def test_empty_compilation_context(buck: Buck) -> None:
    """cc_common.create_compilation_context() with no args creates valid empty context."""
    result = await buck.build("//:empty_context")
    output = result.get_build_report().output_for_target("//:empty_context")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["type"] == "CcCompilationContext"
    assert lines["has_headers"] == "True"
    assert lines["has_includes"] == "True"
    assert lines["has_defines"] == "True"


@buck_test(data_dir="test_create_compilation_context_data")
async def test_merge_compilation_contexts(buck: Buck) -> None:
    """merge_cc_infos merges compilation contexts from 3 CcInfo providers."""
    result = await buck.build("//:merge_contexts")
    output = result.get_build_report().output_for_target("//:merge_contexts")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    # All 3 defines should be present
    assert int(lines["defines_count"]) == 3
    assert "A=1" in lines["defines"]
    assert "B=2" in lines["defines"]
    assert "C=3" in lines["defines"]
    # All 3 include dirs should be present
    assert int(lines["includes_count"]) == 3
    assert "inc_a/" in lines["includes"]
    assert "inc_b/" in lines["includes"]
    assert "inc_c/" in lines["includes"]


@buck_test(data_dir="test_create_compilation_context_data")
async def test_compilation_outputs(buck: Buck) -> None:
    """cc_common.create_compilation_outputs() creates outputs with objects and pic_objects."""
    result = await buck.build("//:compilation_outputs")
    output = result.get_build_report().output_for_target("//:compilation_outputs")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["type"] == "CompilationOutputs"
    assert lines["has_objects"] == "True"
    assert lines["has_pic_objects"] == "True"
    assert int(lines["objects_count"]) == 2
    assert int(lines["pic_objects_count"]) == 1


@buck_test(data_dir="test_create_compilation_context_data")
async def test_ccinfo_provider(buck: Buck) -> None:
    """CcInfo provider can be created with both compilation and linking contexts."""
    result = await buck.build("//:ccinfo_provider")
    output = result.get_build_report().output_for_target("//:ccinfo_provider")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    # type() returns the concrete Starlark type name
    assert "CcInfo" in lines["type"]
    assert lines["has_compilation_context"] == "True"
    assert lines["has_linking_context"] == "True"
    assert lines["comp_type"] == "CcCompilationContext"
    assert "LinkingContext" in lines["link_type"]
