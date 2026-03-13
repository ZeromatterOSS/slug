# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for ctx.actions methods: write, expand_template, run_shell, args, etc."""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_ctx_actions_data")
async def test_actions_write_executable(buck: Buck) -> None:
    """ctx.actions.write() with is_executable=True creates a script file."""
    result = await buck.build("//:write_executable")
    output = result.get_build_report().output_for_target("//:write_executable")
    content = output.read_text().strip()
    assert "executable_output" in content, f"Expected 'executable_output' in '{content}'"


@buck_test(data_dir="test_ctx_actions_data")
async def test_actions_expand_template(buck: Buck) -> None:
    """ctx.actions.expand_template() substitutes placeholders in template files."""
    result = await buck.build("//:template_expansion")
    output = result.get_build_report().output_for_target("//:template_expansion")
    content = output.read_text().strip()
    assert "developer" in content, f"Expected 'developer' in '{content}'"
    assert "kuro" in content, f"Expected 'kuro' in '{content}'"
    assert "9.0" in content, f"Expected '9.0' in '{content}'"


@buck_test(data_dir="test_ctx_actions_data")
async def test_actions_run_shell_string(buck: Buck) -> None:
    """ctx.actions.run_shell() with string command executes shell commands."""
    result = await buck.build("//:shell_string")
    output = result.get_build_report().output_for_target("//:shell_string")
    content = output.read_text().strip()
    assert content == "shell_string_output", f"Expected 'shell_string_output', got '{content}'"


@buck_test(data_dir="test_ctx_actions_data")
async def test_actions_run_shell_inputs(buck: Buck) -> None:
    """ctx.actions.run_shell() with inputs concatenates input files."""
    result = await buck.build("//:shell_inputs")
    output = result.get_build_report().output_for_target("//:shell_inputs")
    content = output.read_text().strip()
    assert "content_a" in content, f"Expected 'content_a' in '{content}'"
    assert "content_b" in content, f"Expected 'content_b' in '{content}'"


@buck_test(data_dir="test_ctx_actions_data")
async def test_actions_declare_directory(buck: Buck) -> None:
    """ctx.actions.declare_directory() creates a directory output."""
    result = await buck.build("//:declared_dir")
    output = result.get_build_report().output_for_target("//:declared_dir")
    content = output.read_text().strip()
    assert content == "ok", f"Expected 'ok', got '{content}'"


@buck_test(data_dir="test_ctx_actions_data")
async def test_actions_write_multiline(buck: Buck) -> None:
    """ctx.actions.write() with multi-line content preserves all lines."""
    result = await buck.build("//:multiline_write")
    output = result.get_build_report().output_for_target("//:multiline_write")
    lines = output.read_text().strip().splitlines()
    assert len(lines) == 3, f"Expected 3 lines, got {len(lines)}"
    assert lines[0] == "line_one", f"Expected 'line_one', got '{lines[0]}'"
    assert lines[1] == "line_two", f"Expected 'line_two', got '{lines[1]}'"
    assert lines[2] == "line_three", f"Expected 'line_three', got '{lines[2]}'"


@buck_test(data_dir="test_ctx_actions_data")
async def test_ctx_label_attributes(buck: Buck) -> None:
    """ctx.label provides name and package attributes."""
    result = await buck.build("//:label_info")
    output = result.get_build_report().output_for_target("//:label_info")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["name"] == "label_info", f"Expected 'label_info', got '{lines['name']}'"
    # Root package should be empty
    assert lines["package"] == "", f"Expected empty package, got '{lines['package']}'"


@buck_test(data_dir="test_ctx_actions_data")
async def test_ctx_bin_dir(buck: Buck) -> None:
    """ctx.bin_dir.path returns a valid output directory path."""
    result = await buck.build("//:bin_dir_test")
    output = result.get_build_report().output_for_target("//:bin_dir_test")
    content = output.read_text().strip()
    # Should be a path containing "buck-out" or "bazel-out"
    assert "out" in content.lower(), f"Expected output path, got '{content}'"


@buck_test(data_dir="test_ctx_actions_data")
async def test_ctx_runfiles(buck: Buck) -> None:
    """ctx.runfiles() collects data files for runtime."""
    result = await buck.build("//:runfiles_test")
    output = result.get_build_report().output_for_target("//:runfiles_test")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["count"] == "2", f"Expected 2 data files, got '{lines['count']}'"


@buck_test(data_dir="test_ctx_actions_data")
async def test_provider_propagation(buck: Buck) -> None:
    """Custom providers propagate through deps and can be accessed via target[Provider]."""
    result = await buck.build("//:provider_consumer")
    output = result.get_build_report().output_for_target("//:provider_consumer")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["val"] == "my_provider_value", f"Expected 'my_provider_value', got '{lines['val']}'"
    assert lines["count"] == "42", f"Expected '42', got '{lines['count']}'"
