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


@buck_test(data_dir="test_cmd_args_data")
async def test_ctx_actions_args_builder(buck: Buck) -> None:
    result = await buck.build("//:args_builder")
    output = result.get_build_report().output_for_target("//:args_builder")

    content = output.read_text().strip().splitlines()
    assert content == ["one", "two", "three", "four,five"]


@buck_test(data_dir="test_cmd_args_data")
async def test_args_add_all_terminate_with(buck: Buck) -> None:
    """args.add_all with terminate_with appends a final element after the list."""
    result = await buck.build("//:args_terminate_with")
    output = result.get_build_report().output_for_target("//:args_terminate_with")

    content = output.read_text().strip().splitlines()
    assert content == ["a", "b", "c", "END"]


@buck_test(data_dir="test_cmd_args_data")
async def test_args_add_all_before_each(buck: Buck) -> None:
    """args.add_all with before_each prepends a string before each element."""
    result = await buck.build("//:args_before_each")
    output = result.get_build_report().output_for_target("//:args_before_each")

    content = output.read_text().strip().splitlines()
    assert content == ["--flag", "a", "--flag", "b"]


@buck_test(data_dir="test_cmd_args_data")
async def test_args_add_all_format_each(buck: Buck) -> None:
    """args.add_all with format_each applies a format string to each element."""
    result = await buck.build("//:args_format_each")
    output = result.get_build_report().output_for_target("//:args_format_each")

    content = output.read_text().strip().splitlines()
    assert content == ["--lib=foo", "--lib=bar"]


@buck_test(data_dir="test_cmd_args_data")
async def test_args_add_all_map_each(buck: Buck) -> None:
    """args.add_all with map_each applies a Starlark function to each element."""
    result = await buck.build("//:args_map_each")
    output = result.get_build_report().output_for_target("//:args_map_each")

    content = output.read_text().strip().splitlines()
    assert content == ["HELLO", "WORLD"]


@buck_test(data_dir="test_cmd_args_data")
async def test_args_add_all_uniquify(buck: Buck) -> None:
    """args.add_all with uniquify=True deduplicates the list while preserving order."""
    result = await buck.build("//:args_uniquify")
    output = result.get_build_report().output_for_target("//:args_uniquify")

    content = output.read_text().strip().splitlines()
    assert content == ["a", "b", "c"]


@buck_test(data_dir="test_cmd_args_data")
async def test_args_add_all_omit_if_empty(buck: Buck) -> None:
    """args.add_all with omit_if_empty=True adds nothing for an empty list."""
    result = await buck.build("//:args_omit_if_empty")
    output = result.get_build_report().output_for_target("//:args_omit_if_empty")

    content = output.read_text().strip().splitlines()
    assert content == ["before", "after"]


@buck_test(data_dir="test_cmd_args_data")
async def test_args_add_two_arg_form(buck: Buck) -> None:
    """args.add("--flag", value) adds two separate arguments."""
    result = await buck.build("//:args_add_two_arg")
    output = result.get_build_report().output_for_target("//:args_add_two_arg")

    content = output.read_text().strip().splitlines()
    assert content == ["--output", "foo.o", "--verbose"]


@buck_test(data_dir="test_cmd_args_data")
async def test_args_add_all_two_arg_form(buck: Buck) -> None:
    """args.add_all("--flag", values) adds flag once then all values."""
    result = await buck.build("//:args_add_all_two_arg")
    output = result.get_build_report().output_for_target("//:args_add_all_two_arg")

    content = output.read_text().strip().splitlines()
    assert content == ["--src", "a.c", "b.c", "c.c"]


@buck_test(data_dir="test_cmd_args_data")
async def test_args_add_joined_two_arg_form(buck: Buck) -> None:
    """args.add_joined("--flag", values, join_with=...) adds flag then joined values as separate args."""
    result = await buck.build("//:args_add_joined_two_arg")
    output = result.get_build_report().output_for_target("//:args_add_joined_two_arg")

    content = output.read_text().strip().splitlines()
    assert content == ["--srcs", "a.c,b.c,c.c"]


@buck_test(data_dir="test_cmd_args_data")
async def test_args_add_joined_uniquify(buck: Buck) -> None:
    """args.add_joined with uniquify=True deduplicates values before joining."""
    result = await buck.build("//:args_add_joined_uniquify")
    output = result.get_build_report().output_for_target("//:args_add_joined_uniquify")

    content = output.read_text().strip()
    assert content == "a,b,c"


@buck_test(data_dir="test_cmd_args_data")
async def test_args_add_format_with_artifact(buck: Buck) -> None:
    """args.add with format= applies a format string to an artifact path."""
    result = await buck.build("//:args_output_artifact")
    output = result.get_build_report().output_for_target("//:args_output_artifact")

    content = output.read_text().strip()
    assert content.startswith("--input=")
    assert "defs.bzl" in content
