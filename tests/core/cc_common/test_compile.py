# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""Tests for cc_common.compile() API."""

import pytest

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_compile_data")
async def test_compile_basic(buck: Buck) -> None:
    """cc_common.compile() creates compilation context and outputs from source files."""
    result = await buck.build("//:compile_basic")
    output = result.get_build_report().output_for_target("//:compile_basic")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["comp_ctx_type"] == "CcCompilationContext"
    assert lines["outputs_type"] == "CompilationOutputs"
    assert lines["has_objects"] == "True"
    assert lines["has_pic_objects"] == "True"
    # Compilation context should have standard attributes
    assert lines["has_headers"] == "True"
    assert lines["has_includes"] == "True"
    assert lines["has_defines"] == "True"
    assert lines["has_direct_headers"] == "True"


@buck_test(data_dir="test_compile_data")
async def test_compile_with_defines(buck: Buck) -> None:
    """cc_common.compile() passes defines to the compilation context."""
    result = await buck.build("//:compile_with_defines")
    output = result.get_build_report().output_for_target("//:compile_with_defines")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["comp_ctx_type"] == "CcCompilationContext"
    # Should have at least the defines we passed
    defines = lines.get("defines", "")
    assert "MY_DEFINE" in defines or int(lines.get("defines_count", "0")) >= 1


@buck_test(data_dir="test_compile_data")
async def test_compile_with_includes(buck: Buck) -> None:
    """cc_common.compile() passes include directories to the compilation context."""
    result = await buck.build("//:compile_with_includes")
    output = result.get_build_report().output_for_target("//:compile_with_includes")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    includes = lines.get("includes", "")
    assert "myinc/" in includes
    quote_includes = lines.get("quote_includes", "")
    assert "myquote/" in quote_includes
    system_includes = lines.get("system_includes", "")
    assert "mysystem/" in system_includes


@buck_test(data_dir="test_compile_data")
async def test_compile_with_flags(buck: Buck) -> None:
    """cc_common.compile() accepts user_compile_flags without error."""
    result = await buck.build("//:compile_with_flags")
    output = result.get_build_report().output_for_target("//:compile_with_flags")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["comp_ctx_type"] == "CcCompilationContext"
    assert lines["outputs_type"] == "CompilationOutputs"


@buck_test(data_dir="test_compile_data")
async def test_compile_multiple_srcs(buck: Buck) -> None:
    """cc_common.compile() handles multiple source files (C and C++)."""
    result = await buck.build("//:compile_multiple_srcs")
    output = result.get_build_report().output_for_target("//:compile_multiple_srcs")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["comp_ctx_type"] == "CcCompilationContext"


@buck_test(data_dir="test_compile_data")
async def test_compile_dep_contexts(buck: Buck) -> None:
    """cc_common.compile() accepts compilation_contexts from deps."""
    result = await buck.build("//:compile_dep_contexts")
    output = result.get_build_report().output_for_target("//:compile_dep_contexts")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["comp_ctx_type"] == "CcCompilationContext"
    assert lines["outputs_type"] == "CompilationOutputs"
