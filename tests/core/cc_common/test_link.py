# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""Tests for cc_common.link() and related linking APIs."""

import pytest

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_link_data")
async def test_link_executable(buck: Buck) -> None:
    """cc_common.link() with output_type='executable' returns CcLinkingOutputs."""
    result = await buck.build("//:link_executable")
    output = result.get_build_report().output_for_target("//:link_executable")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["type"] == "CcLinkingOutputs"
    assert lines["has_library_to_link"] == "True"
    assert lines["has_executable"] == "True"


@buck_test(data_dir="test_link_data")
async def test_link_dynamic_library(buck: Buck) -> None:
    """cc_common.link() with output_type='dynamic_library' returns CcLinkingOutputs."""
    result = await buck.build("//:link_dynamic_library")
    output = result.get_build_report().output_for_target("//:link_dynamic_library")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["type"] == "CcLinkingOutputs"
    assert lines["has_library_to_link"] == "True"


@buck_test(data_dir="test_link_data")
async def test_link_with_user_flags(buck: Buck) -> None:
    """cc_common.link() accepts user_link_flags without error."""
    result = await buck.build("//:link_with_user_flags")
    output = result.get_build_report().output_for_target("//:link_with_user_flags")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["type"] == "CcLinkingOutputs"
    assert lines["has_executable"] == "True"


@buck_test(data_dir="test_link_data")
async def test_link_with_linking_contexts(buck: Buck) -> None:
    """cc_common.link() accepts linking_contexts from deps."""
    result = await buck.build("//:link_with_linking_contexts")
    output = result.get_build_report().output_for_target("//:link_with_linking_contexts")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["type"] == "CcLinkingOutputs"
    assert lines["has_executable"] == "True"


@buck_test(data_dir="test_link_data")
async def test_create_library_to_link(buck: Buck) -> None:
    """cc_common.create_library_to_link() creates a LibraryToLink with expected fields."""
    result = await buck.build("//:create_library_to_link")
    output = result.get_build_report().output_for_target("//:create_library_to_link")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["type"] == "LibraryToLink"
    assert lines["has_static_library"] == "True"
    assert lines["has_dynamic_library"] == "True"
    assert lines["has_pic_static_library"] == "True"


@buck_test(data_dir="test_link_data")
async def test_linker_input(buck: Buck) -> None:
    """cc_common.create_linker_input() preserves user_link_flags and owner."""
    result = await buck.build("//:linker_input")
    output = result.get_build_report().output_for_target("//:linker_input")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["type"] == "LinkerInput"
    assert lines["has_user_link_flags"] == "True"
    assert int(lines["flags_count"]) == 3
    assert "-lcrypto" in lines["flags"]
    assert "-lssl" in lines["flags"]
    assert "-L/usr/local/lib" in lines["flags"]
    assert lines["has_owner"] == "True"
