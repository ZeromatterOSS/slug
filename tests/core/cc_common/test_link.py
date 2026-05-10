# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

"""Tests for cc_common.link() and related linking APIs."""

import pytest

from buck2.tests.e2e_util.asserts import expect_failure
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
async def test_link_deps_statically(buck: Buck) -> None:
    """cc_common.link() respects link_deps_statically parameter."""
    result = await buck.build("//:link_deps_statically")
    output = result.get_build_report().output_for_target("//:link_deps_statically")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["static_type"] == "CcLinkingOutputs"
    assert lines["static_has_executable"] == "True"
    assert lines["dynamic_type"] == "CcLinkingOutputs"
    assert lines["dynamic_has_executable"] == "True"


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


@buck_test(data_dir="test_link_data")
async def test_linker_input_nested_user_flags_depset_element(buck: Buck) -> None:
    """LinkerInput with nested user_link_flags is immutable enough for depset membership."""
    result = await buck.build("//:linker_input_nested_user_flags")
    output = result.get_build_report().output_for_target("//:linker_input_nested_user_flags")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["type"] == "LinkingContext"
    assert lines["inputs_count"] == "1"
    assert lines["flags"] == "-lcrypto,-lssl,-lz"


@buck_test(data_dir="test_link_data")
async def test_frozen_dict_depset_element(buck: Buck) -> None:
    """cc_internal.freeze preserves dict APIs while allowing depset membership."""
    result = await buck.build("//:frozen_dict_depset")
    output = result.get_build_report().output_for_target("//:frozen_dict_depset")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["payloads_count"] == "1"
    assert lines["payload_type"] == "dict"
    assert lines["payload_truthy"] == "True"
    assert lines["payload_keys"] == "backend"
    assert lines["payload_get_type"] == "tuple"
    assert lines["payload_get_len"] == "2"
    assert lines["payload_contains_backend"] == "True"
    assert lines["payload_iter"] == "backend"
    assert lines["merged_get_len"] == "2"


@buck_test(data_dir="test_link_data")
async def test_starlark_library_to_link_provider_depset_element(buck: Buck) -> None:
    """rules_cc-shaped LibraryToLink providers with frozen fields are depset-safe."""
    result = await buck.build("//:starlark_library_to_link_depset")
    output = result.get_build_report().output_for_target("//:starlark_library_to_link_depset")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["libraries_count"] == "1"
    assert lines["static_basename"] == "lib.c"
    assert lines["lto_inputs_count"] == "0"


@buck_test(data_dir="test_link_data")
async def test_starlark_library_to_link_provider_with_mutable_field_rejected(
    buck: Buck,
) -> None:
    """Analysis-time mutable provider fields still fail depset validation."""
    await expect_failure(
        buck.build("//:mutable_library_to_link_depset"),
        stderr_regex="depset elements must not be mutable values",
    )
