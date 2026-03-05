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


@buck_test(data_dir="test_native_rules_data")
async def test_filegroup_collects_srcs(buck: Buck) -> None:
    """filegroup collects source files and makes them available as deps."""
    result = await buck.build("//:collected_filegroup")
    output = result.get_build_report().output_for_target("//:collected_filegroup")

    content = output.read_text().strip().splitlines()
    assert "defs.bzl" in content
    assert "MODULE.bazel" in content


@buck_test(data_dir="test_native_rules_data")
async def test_alias_resolves_to_original(buck: Buck) -> None:
    """alias() creates an alternative name that resolves to the same outputs."""
    result = await buck.build("//:aliased")
    output = result.get_build_report().output_for_target("//:aliased")

    content = output.read_text().strip().splitlines()
    assert content == ["hello", "world"]


@buck_test(data_dir="test_native_rules_data")
async def test_select_conditions_default(buck: Buck) -> None:
    """select() with //conditions:default always matches."""
    result = await buck.build("//:select_default")
    output = result.get_build_report().output_for_target("//:select_default")

    content = output.read_text().strip()
    assert content == "default_value"


@buck_test(data_dir="test_native_rules_data")
async def test_select_with_constraint_values_default(buck: Buck) -> None:
    """select() with constraint_values config_settings falls back to default
    when no platform is specified."""
    result = await buck.build("//:select_with_constraint")
    output = result.get_build_report().output_for_target("//:select_with_constraint")

    content = output.read_text().strip()
    assert content == "default_selected"


@buck_test(data_dir="test_native_rules_data")
async def test_constraint_setting_and_value_build(buck: Buck) -> None:
    """constraint_setting and constraint_value rules can be defined and build."""
    # These build successfully as analysis-only rules (no outputs)
    await buck.build(
        "//:my_setting",
        "//:my_value_a",
        "//:my_value_b",
    )


@buck_test(data_dir="test_native_rules_data")
async def test_platform_builds(buck: Buck) -> None:
    """platform() rule can be defined and builds successfully."""
    await buck.build(
        "//:linux_platform",
        "//:macos_platform",
    )


@buck_test(data_dir="test_native_rules_data")
async def test_config_setting_builds(buck: Buck) -> None:
    """config_setting() rule can be defined and builds successfully."""
    await buck.build(
        "//:config_a",
        "//:config_b",
    )


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_basic(buck: Buck) -> None:
    """genrule() executes cmd and produces output file."""
    result = await buck.build("//:genrule_basic")
    output = result.get_build_report().output_for_target("//:genrule_basic")
    content = output.read_text().strip()
    assert content == "hello from genrule"


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_srcs_expansion(buck: Buck) -> None:
    """genrule $(SRCS) expands to project-relative execpaths of input files."""
    result = await buck.build("//:genrule_srcs")
    output = result.get_build_report().output_for_target("//:genrule_srcs")
    content = output.read_text().strip()
    # $(SRCS) expands to the execpath of defs.bzl (project-relative, no drive/spaces).
    # For a root-package source, execpath is just the filename with no directory prefix.
    assert content.endswith("defs.bzl")


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_multi_outputs(buck: Buck) -> None:
    """genrule with multiple outs uses $(@D) to write to output directory."""
    result = await buck.build("//:genrule_multi_outs")
    outputs = result.get_build_report().outputs_for_target("//:genrule_multi_outs")
    by_name = {p.name: p for p in outputs}
    assert by_name["multi_a.txt"].read_text().strip() == "a"
    assert by_name["multi_b.txt"].read_text().strip() == "b"


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_cmd_bash_preferred_on_unix(buck: Buck) -> None:
    """genrule uses cmd_bash over cmd on Unix/Linux platforms."""
    result = await buck.build("//:genrule_cmd_bash")
    output = result.get_build_report().output_for_target("//:genrule_cmd_bash")
    content = output.read_text().strip()
    # cmd_bash="echo 'from_bash' > $@" should take priority over cmd="echo 'generic' > $@"
    assert content == "from_bash"


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_location_expansion(buck: Buck) -> None:
    """genrule $(location :file) expands to the project-relative execpath of a source file."""
    result = await buck.build("//:genrule_location")
    output = result.get_build_report().output_for_target("//:genrule_location")
    content = output.read_text().strip()
    # $(location :defs.bzl) expands to the execpath of defs.bzl (project-relative).
    # For a root-package source, execpath is just the filename with no directory prefix.
    assert content.endswith("defs.bzl")


@buck_test(data_dir="test_native_rules_data")
async def test_sh_binary_builds(buck: Buck) -> None:
    """sh_binary() rule can be defined and builds successfully."""
    await buck.build("//:hello_sh")


@buck_test(data_dir="test_native_rules_data")
async def test_sh_test_runs(buck: Buck) -> None:
    """sh_test() runs successfully using bash as interpreter."""
    result = await buck.test("//:hello_sh_test")
    assert result.get_success_count() > 0
    assert result.get_failure_count() == 0


@buck_test(data_dir="test_native_rules_data")
async def test_test_suite_builds(buck: Buck) -> None:
    """test_suite() can group tests and builds successfully."""
    await buck.build("//:all_sh_tests")


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_select_cmd(buck: Buck) -> None:
    """genrule cmd attribute accepts select() expressions."""
    result = await buck.build("//:genrule_select_cmd")
    output = result.get_build_report().output_for_target("//:genrule_select_cmd")
    content = output.read_text().strip()
    # With //conditions:default, the default branch is always selected
    assert content == "selected_default"
