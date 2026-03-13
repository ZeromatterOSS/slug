# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for genrule features: cmd variants, Make variables, select(), tools."""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_genrule_features_data")
async def test_basic_genrule(buck: Buck) -> None:
    """Basic genrule with cmd and single output ($@)."""
    result = await buck.build("//:basic_genrule")
    output = result.get_build_report().output_for_target("//:basic_genrule")
    content = output.read_text().strip()
    assert content == "basic_output"


@buck_test(data_dir="test_genrule_features_data")
async def test_genrule_with_srcs(buck: Buck) -> None:
    """genrule with srcs: $(SRCS) and $< expand to input file paths."""
    result = await buck.build("//:genrule_with_srcs")
    output = result.get_build_report().output_for_target("//:genrule_with_srcs")
    content = output.read_text().strip()
    assert content == "input_content_here"


@buck_test(data_dir="test_genrule_features_data")
async def test_genrule_multi_out(buck: Buck) -> None:
    """genrule with multiple outs: $(@D) resolves to output directory."""
    result = await buck.build("//:multi_out")
    outputs = result.get_build_report().outputs_for_target("//:multi_out")
    names = {p.name for p in outputs}
    assert "out_a.txt" in names, f"Expected out_a.txt in {names}"
    assert "out_b.txt" in names, f"Expected out_b.txt in {names}"


@buck_test(data_dir="test_genrule_features_data")
async def test_genrule_cmd_bash(buck: Buck) -> None:
    """genrule with cmd_bash: bash-specific command takes priority on Unix."""
    import sys
    result = await buck.build("//:bash_genrule")
    output = result.get_build_report().output_for_target("//:bash_genrule")
    content = output.read_text().strip()
    if sys.platform != "win32":
        assert content == "bash_specific", f"Expected 'bash_specific', got '{content}'"
    else:
        # On Windows, cmd is used if cmd_bash is not the preferred shell
        assert content in ("bash_specific", "fallback"), f"Unexpected: '{content}'"


@buck_test(data_dir="test_genrule_features_data")
async def test_genrule_location_tool(buck: Buck) -> None:
    """genrule with tools= and $(location :tool) resolves to tool output."""
    result = await buck.build("//:tool_genrule")
    output = result.get_build_report().output_for_target("//:tool_genrule")
    content = output.read_text().strip()
    assert content == "basic_output"


@buck_test(data_dir="test_genrule_features_data")
async def test_genrule_select_default(buck: Buck) -> None:
    """genrule with select() on cmd: defaults to default condition."""
    result = await buck.build("//:select_genrule")
    output = result.get_build_report().output_for_target("//:select_genrule")
    content = output.read_text().strip()
    assert content == "default"


@buck_test(data_dir="test_genrule_features_data")
async def test_genrule_select_opt(buck: Buck) -> None:
    """genrule with select() matches opt config_setting with --compilation_mode=opt."""
    result = await buck.build("//:select_genrule", "--compilation_mode=opt")
    output = result.get_build_report().output_for_target("//:select_genrule")
    content = output.read_text().strip()
    assert content == "optimized"


@buck_test(data_dir="test_genrule_features_data")
async def test_genrule_outs_var(buck: Buck) -> None:
    """genrule $(OUTS) expands to space-separated output paths."""
    result = await buck.build("//:outs_var_genrule")
    output = result.get_build_report().output_for_target("//:outs_var_genrule")
    content = output.read_text().strip()
    assert content == "content"


@buck_test(data_dir="test_genrule_features_data")
async def test_genrule_ruledir(buck: Buck) -> None:
    """genrule $(RULEDIR) expands to the output directory."""
    result = await buck.build("//:ruledir_genrule")
    output = result.get_build_report().output_for_target("//:ruledir_genrule")
    content = output.read_text().strip()
    assert content == "ok"
