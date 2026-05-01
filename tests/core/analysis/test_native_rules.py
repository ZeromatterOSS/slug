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
async def test_select_no_match_error_param(buck: Buck) -> None:
    """select() accepts no_match_error parameter (Bazel-compatible)."""
    result = await buck.build("//:select_no_match_error")
    output = result.get_build_report().output_for_target("//:select_no_match_error")

    content = output.read_text().strip()
    assert content == "matched_default"


@buck_test(data_dir="test_native_rules_data")
async def test_select_with_constraint_values_default(buck: Buck) -> None:
    """select() with constraint_values config_settings falls back to default
    when no platform is specified."""
    result = await buck.build("//:select_with_constraint")
    output = result.get_build_report().output_for_target("//:select_with_constraint")

    content = output.read_text().strip()
    assert content == "default_selected"


@buck_test(data_dir="test_native_rules_data")
async def test_select_constraint_values_with_platform(buck: Buck) -> None:
    """select() matches config_setting(constraint_values=...) when --target-platforms is set."""
    result = await buck.build(
        "//:select_by_platform",
        "--target-platforms=//:linux_platform",
    )
    output = result.get_build_report().output_for_target("//:select_by_platform")
    content = output.read_text().strip()
    assert content == "linux_matched"


@buck_test(data_dir="test_native_rules_data")
async def test_select_constraint_values_with_different_platform(buck: Buck) -> None:
    """select() matches different config_setting when a different platform is used."""
    result = await buck.build(
        "//:select_by_platform",
        "--target-platforms=//:macos_platform",
    )
    output = result.get_build_report().output_for_target("//:select_by_platform")
    content = output.read_text().strip()
    assert content == "macos_matched"


@buck_test(data_dir="test_native_rules_data")
async def test_select_constraint_values_custom_platform(buck: Buck) -> None:
    """select() correctly matches config_a when platform has my_value_a constraint."""
    result = await buck.build(
        "//:select_with_constraint",
        "--target-platforms=//:platform_with_value_a",
    )
    output = result.get_build_report().output_for_target("//:select_with_constraint")
    content = output.read_text().strip()
    assert content == "config_a_selected"


@buck_test(data_dir="test_native_rules_data")
async def test_select_by_compilation_mode_default(buck: Buck) -> None:
    """select() with config_setting(values={"compilation_mode":"fastbuild"}) matches default mode."""
    result = await buck.build("//:select_by_compilation_mode")
    output = result.get_build_report().output_for_target("//:select_by_compilation_mode")
    content = output.read_text().strip()
    assert content == "fastbuild_selected"


@buck_test(data_dir="test_native_rules_data")
async def test_select_by_compilation_mode_opt(buck: Buck) -> None:
    """select() matches config_setting(values={"compilation_mode":"opt"}) with --compilation_mode=opt."""
    result = await buck.build("//:select_by_compilation_mode", "--compilation_mode=opt")
    output = result.get_build_report().output_for_target("//:select_by_compilation_mode")
    content = output.read_text().strip()
    assert content == "opt_selected"


@buck_test(data_dir="test_native_rules_data")
async def test_select_by_define_default(buck: Buck) -> None:
    """select() with config_setting(define_values={}) uses default when --define not set."""
    result = await buck.build("//:select_by_define")
    output = result.get_build_report().output_for_target("//:select_by_define")
    content = output.read_text().strip()
    assert content == "no_define"


@buck_test(data_dir="test_native_rules_data")
async def test_select_by_define_match(buck: Buck) -> None:
    """select() matches config_setting(define_values={"FOO":"bar"}) with --define=FOO=bar."""
    result = await buck.build("//:select_by_define", "--define=FOO=bar")
    output = result.get_build_report().output_for_target("//:select_by_define")
    content = output.read_text().strip()
    assert content == "foo_is_bar"


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
async def test_sh_library_builds(buck: Buck) -> None:
    """sh_library() rule can be defined and builds successfully."""
    await buck.build("//:hello_sh_lib")


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


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_with_tool(buck: Buck) -> None:
    """genrule tools= attribute makes executables available via $(location)."""
    result = await buck.build("//:genrule_with_tool")
    output = result.get_build_report().output_for_target("//:genrule_with_tool")
    # The genrule runs the sh_binary tool via bash, which outputs "hello from shell script"
    content = output.read_text().strip()
    assert "hello" in content


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_execpath_expansion(buck: Buck) -> None:
    """genrule $(execpath :label) expands to the execpath of a source file (alias for $(location))."""
    result = await buck.build("//:genrule_execpath")
    output = result.get_build_report().output_for_target("//:genrule_execpath")
    content = output.read_text().strip()
    assert content.endswith("defs.bzl"), f"Expected path ending with defs.bzl, got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_rootpath_expansion(buck: Buck) -> None:
    """genrule $(rootpath :label) expands to the runfiles-relative path of a source file."""
    result = await buck.build("//:genrule_rootpath")
    output = result.get_build_report().output_for_target("//:genrule_rootpath")
    content = output.read_text().strip()
    assert content.endswith("defs.bzl"), f"Expected path ending with defs.bzl, got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_rootpaths_expansion(buck: Buck) -> None:
    """genrule $(rootpaths :label) expands to space-separated runfiles-relative paths."""
    result = await buck.build("//:genrule_rootpaths")
    output = result.get_build_report().output_for_target("//:genrule_rootpaths")
    content = output.read_text().strip()
    assert content.endswith("defs.bzl"), f"Expected path ending with defs.bzl, got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_rlocationpath_expansion(buck: Buck) -> None:
    """genrule $(rlocationpath :label) expands to the rlocation path of a source file."""
    result = await buck.build("//:genrule_rlocationpath")
    output = result.get_build_report().output_for_target("//:genrule_rlocationpath")
    content = output.read_text().strip()
    assert content.endswith("defs.bzl"), f"Expected path ending with defs.bzl, got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_rlocationpaths_expansion(buck: Buck) -> None:
    """genrule $(rlocationpaths :label) expands to space-separated rlocation paths."""
    result = await buck.build("//:genrule_rlocationpaths")
    output = result.get_build_report().output_for_target("//:genrule_rlocationpaths")
    content = output.read_text().strip()
    assert content.endswith("defs.bzl"), f"Expected path ending with defs.bzl, got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_bindir_expansion(buck: Buck) -> None:
    """genrule $(BINDIR) expands to the output directory root (buck-out/...)."""
    result = await buck.build("//:genrule_bindir")
    output = result.get_build_report().output_for_target("//:genrule_bindir")
    content = output.read_text().strip()
    assert "buck-out" in content, f"Expected buck-out in BINDIR expansion, got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
async def test_genrule_ruledir_expansion(buck: Buck) -> None:
    """genrule $(@D) expands to the output directory for the rule."""
    result = await buck.build("//:genrule_ruledir")
    output = result.get_build_report().output_for_target("//:genrule_ruledir")
    content = output.read_text().strip()
    assert "buck-out" in content, f"Expected buck-out in $(@D) expansion, got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
@pytest.mark.skipif(sys.platform != "win32", reason="cmd_ps is Windows-only")
async def test_genrule_cmd_ps_on_windows(buck: Buck) -> None:
    """On Windows, genrule uses cmd_ps (PowerShell) when cmd_ps is provided."""
    result = await buck.build("//:genrule_cmd_ps")
    output = result.get_build_report().output_for_target("//:genrule_cmd_ps")
    content = output.read_text().strip()
    assert content == "from_powershell", f"Expected 'from_powershell', got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
@pytest.mark.skipif(sys.platform != "win32", reason="cmd_bat is Windows-only")
async def test_genrule_cmd_bat_on_windows(buck: Buck) -> None:
    """On Windows, genrule uses cmd_bat (CMD.exe) when cmd_bat is provided but not cmd_ps."""
    result = await buck.build("//:genrule_cmd_bat")
    output = result.get_build_report().output_for_target("//:genrule_cmd_bat")
    content = output.read_text().strip()
    assert content == "from_cmd", f"Expected 'from_cmd', got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
@pytest.mark.skipif(sys.platform != "win32", reason="cmd_ps/cmd_bat priority is Windows-only")
async def test_genrule_cmd_ps_priority_on_windows(buck: Buck) -> None:
    """On Windows, cmd_ps takes priority over cmd_bat when both are provided."""
    result = await buck.build("//:genrule_cmd_ps_priority")
    output = result.get_build_report().output_for_target("//:genrule_cmd_ps_priority")
    content = output.read_text().strip()
    assert content == "from_ps_wins", f"Expected 'from_ps_wins', got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
async def test_select_resolves_with_target_platforms(buck: Buck) -> None:
    """select() correctly resolves to platform-specific branch with --target-platforms."""
    result = await buck.build(
        "//:select_platform",
        "--target-platforms=//:linux_platform",
    )
    output = result.get_build_report().output_for_target("//:select_platform")
    content = output.read_text().strip()
    assert content == "linux_selected", f"Expected 'linux_selected', got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
async def test_config_bool_build_setting(buck: Buck) -> None:
    """config.bool() build settings work with flag_values in config_setting for select()."""
    result = await buck.build("//:select_by_bool_flag")
    output = result.get_build_report().output_for_target("//:select_by_bool_flag")
    content = output.read_text().strip()
    # Default is False, so flag_is_false config_setting should match
    assert content == "flag_is_false", f"Expected 'flag_is_false', got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
async def test_config_string_build_setting(buck: Buck) -> None:
    """config.string() build settings work with flag_values in config_setting for select()."""
    result = await buck.build("//:select_by_string_flag")
    output = result.get_build_report().output_for_target("//:select_by_string_flag")
    content = output.read_text().strip()
    # Default is "default_val", so string_flag_is_default should match
    assert content == "default_selected", f"Expected 'default_selected', got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
async def test_label_flag_with_flag_values(buck: Buck) -> None:
    """label_flag build settings work with flag_values in config_setting for select()."""
    result = await buck.build("//:select_by_label_flag")
    output = result.get_build_report().output_for_target("//:select_by_label_flag")
    content = output.read_text().strip()
    # config_setting with flag_values matching the label_flag's default should select
    assert content == "default_flag_selected", f"Expected 'default_flag_selected', got: {content!r}"


@buck_test(data_dir="test_native_rules_data")
async def test_toolchain_type_builds(buck: Buck) -> None:
    """toolchain_type() rule can be defined and builds successfully."""
    await buck.build("//:my_toolchain_type")


@buck_test(data_dir="test_native_rules_data")
async def test_toolchain_builds(buck: Buck) -> None:
    """toolchain() rule can be defined and builds successfully."""
    await buck.build("//:my_toolchain")


@buck_test(data_dir="test_native_rules_data")
async def test_package_group_builds(buck: Buck) -> None:
    """package_group() rule builds successfully."""
    await buck.build("//:all_packages")
    await buck.build("//:root_only")


@buck_test(data_dir="test_native_rules_data")
async def test_target_with_package_group_visibility(buck: Buck) -> None:
    """genrule with package_group visibility builds successfully."""
    result = await buck.build("//:pkg_group_target")
    output = result.get_build_report().output_for_target("//:pkg_group_target")
    content = output.read_text().strip()
    assert "package_group_ok" in content


@buck_test(data_dir="test_native_rules_data")
async def test_package_group_cross_package_visibility(buck: Buck) -> None:
    """Target in subpackage can depend on target visible via package_group."""
    result = await buck.build("//subpkg:cross_pkg_consumer")
    output = result.get_build_report().output_for_target("//subpkg:cross_pkg_consumer")
    content = output.read_text().strip()
    assert "visible_via_group" in content


@buck_test(data_dir="test_native_rules_data")
async def test_declare_file_with_sibling(buck: Buck) -> None:
    """declare_file() with sibling places output in sibling's directory."""
    result = await buck.build("//:sibling_test")
    output = result.get_build_report().output_for_target("//:sibling_test")
    content = output.read_text().strip()
    assert content == "sibling", f"Expected 'sibling', got: {content!r}"
    # The sibling file should be in the same subdirectory as the original
    assert "subdir" in str(output), f"Expected 'subdir' in output path: {output}"


@buck_test(data_dir="test_native_rules_data")
async def test_stamp_files_are_file_objects(buck: Buck) -> None:
    """ctx.info_file and ctx.version_file return File-like objects."""
    result = await buck.build("//:stamp_info")
    output = result.get_build_report().output_for_target("//:stamp_info")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    # Both should be File type
    assert lines["info_type"] == "File", f"Expected File type, got: {lines['info_type']}"
    assert lines["version_type"] == "File", f"Expected File type, got: {lines['version_type']}"
    # Paths should be correct
    assert "stable-status.txt" in lines["info_path"]
    assert "volatile-status.txt" in lines["version_path"]
    # Short paths
    assert lines["info_short_path"] == "stable-status.txt"
    assert lines["version_short_path"] == "volatile-status.txt"
    # Basenames
    assert lines["info_basename"] == "stable-status.txt"
    assert lines["version_basename"] == "volatile-status.txt"


@buck_test(data_dir="test_native_rules_data")
async def test_rule_exec_groups(buck: Buck) -> None:
    """rule(exec_groups={...}) is accepted and ctx.exec_groups works."""
    result = await buck.build("//:exec_groups_test")
    output = result.get_build_report().output_for_target("//:exec_groups_test")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    assert lines["type"] == "exec_groups"
    assert lines["has_compile"] == "True"
    assert lines["has_link"] == "True"
    assert lines["has_toolchains"] == "True"


@buck_test(data_dir="test_native_rules_data")
async def test_rule_fragments(buck: Buck) -> None:
    """rule(fragments=["cpp"]) is accepted and ctx.fragments.cpp works."""
    result = await buck.build("//:fragments_test")
    output = result.get_build_report().output_for_target("//:fragments_test")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    assert lines["has_cpp"] == "True"
    assert lines["compilation_mode"] in ("fastbuild", "opt", "dbg")


@buck_test(data_dir="test_native_rules_data")
async def test_run_environment_info_provider(buck: Buck) -> None:
    """RunEnvironmentInfo returns a proper provider with environment and inherited_environment."""
    result = await buck.build("//:run_env_test")
    output = result.get_build_report().output_for_target("//:run_env_test")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    assert lines["type"] == "RunEnvironmentInfo"
    assert lines["env_type"] == "dict"
    assert lines["inherited_type"] == "list"


@buck_test(data_dir="test_native_rules_data")
async def test_cc_common_configure_features(buck: Buck) -> None:
    """cc_common.configure_features() respects requested/unsupported features."""
    result = await buck.build("//:cc_configure_features_test")
    output = result.get_build_report().output_for_target("//:cc_configure_features_test")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    assert lines["default_type"] == "FeatureConfiguration"
    # Default features should be enabled
    assert lines["default_supports_dynamic_linker"] == "True"
    assert lines["default_compiler_param_file"] == "True"
    # Custom feature not in defaults should be disabled
    assert lines["default_my_custom"] == "False"
    # Requested features should be enabled
    assert lines["with_custom"] == "True"
    assert lines["with_c++17"] == "True"
    # Unsupported features should be disabled
    assert lines["without_pic"] == "False"
    assert lines["without_supports_pic"] == "False"
    # Other features should still be enabled
    assert lines["without_pic_dynamic_linker"] == "True"


@buck_test(data_dir="test_native_rules_data")
async def test_cc_common_linker_input(buck: Buck) -> None:
    """cc_common.create_linker_input() preserves user_link_flags."""
    result = await buck.build("//:cc_linker_input_test")
    output = result.get_build_report().output_for_target("//:cc_linker_input_test")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    assert lines["type"] == "LinkerInput"
    assert lines["has_user_link_flags"] == "True"
    # Flags should be preserved, not empty
    assert "-lm" in lines["flags_list"]
    assert "-lpthread" in lines["flags_list"]


@buck_test(data_dir="test_native_rules_data")
async def test_cc_common_link(buck: Buck) -> None:
    """cc_common.link() is callable and returns CcLinkingOutputs with expected attributes."""
    result = await buck.build("//:cc_link_test")
    output = result.get_build_report().output_for_target("//:cc_link_test")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    assert lines["type"] == "CcLinkingOutputs"
    assert lines["has_library_to_link"] == "True"
    assert lines["has_executable"] == "True"


@buck_test(data_dir="test_native_rules_data")
async def test_cc_common_create_compilation_context(buck: Buck) -> None:
    """cc_common.create_compilation_context() creates CcCompilationContext with proper attributes."""
    result = await buck.build("//:cc_compilation_context_test")
    output = result.get_build_report().output_for_target("//:cc_compilation_context_test")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    assert lines["type"] == "CcCompilationContext"
    assert lines["has_headers"] == "True"
    assert lines["has_includes"] == "True"
    assert lines["has_defines"] == "True"
    assert lines["has_system_includes"] == "True"
    assert lines["has_direct_headers"] == "True"


@buck_test(data_dir="test_native_rules_data")
async def test_cc_common_merge_cc_infos(buck: Buck) -> None:
    """cc_common.merge_cc_infos() merges CcInfo providers with proper data."""
    result = await buck.build("//:cc_merge_infos_test")
    output = result.get_build_report().output_for_target("//:cc_merge_infos_test")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    assert lines["has_compilation_context"] == "True"
    assert lines["has_linking_context"] == "True"
    assert lines["comp_ctx_type"] == "CcCompilationContext"
    # Verify both CcInfos' defines/includes were merged (not just the last one)
    assert lines["defines_count"] == "2"
    assert lines["defines"] == "DEF1=1,DEF2=2"
    assert lines["includes_count"] == "2"
    assert lines["includes"] == "inc1/,inc2/"


@buck_test(data_dir="test_native_rules_data")
async def test_config_setting_values_compilation_mode(buck: Buck) -> None:
    """config_setting(values={"compilation_mode": "fastbuild"}) matches by default."""
    result = await buck.build("//:select_by_compilation_mode")
    output = result.get_build_report().output_for_target("//:select_by_compilation_mode")
    content = output.read_text().strip()
    # Default compilation mode is "fastbuild", so the fastbuild config_setting should match
    assert content == "fastbuild_selected"


@buck_test(data_dir="test_native_rules_data")
async def test_config_setting_define_values_default(buck: Buck) -> None:
    """config_setting(define_values={"FOO": "bar"}) does not match when no --define given."""
    result = await buck.build("//:select_by_define")
    output = result.get_build_report().output_for_target("//:select_by_define")
    content = output.read_text().strip()
    assert content == "no_define"


@buck_test(data_dir="test_native_rules_data")
async def test_config_setting_define_values_match(buck: Buck) -> None:
    """config_setting(define_values={"FOO": "bar"}) matches when --define FOO=bar given."""
    result = await buck.build("//:select_by_define", "--define", "FOO=bar")
    output = result.get_build_report().output_for_target("//:select_by_define")
    content = output.read_text().strip()
    assert content == "foo_is_bar"


@buck_test(data_dir="test_native_rules_data")
async def test_existing_rules_returns_kind(buck: Buck) -> None:
    """native.existing_rules() returns dicts with 'kind' key for each target."""
    result = await buck.build("//:existing_rules_check")
    output = result.get_build_report().output_for_target("//:existing_rules_check")
    content = output.read_text().strip()
    lines = content.splitlines()
    # Should have entries like "source_files=filegroup", "original=write_list", etc.
    entries = {}
    repo_name = None
    for line in lines:
        if line.startswith("repo="):
            repo_name = line.split("=", 1)[1]
        elif "=" in line:
            name, kind = line.split("=", 1)
            entries[name] = kind
    # Verify some known targets have correct kinds
    assert entries.get("source_files") == "filegroup", f"Expected filegroup, got {entries.get('source_files')}"
    assert entries.get("original") == "write_list", f"Expected write_list, got {entries.get('original')}"
    assert entries.get("aliased") == "alias", f"Expected alias, got {entries.get('aliased')}"
    assert entries.get("genrule_basic") == "genrule", f"Expected genrule, got {entries.get('genrule_basic')}"
    # No target should have MISSING kind
    for name, kind in entries.items():
        assert kind != "MISSING", f"Target {name} has MISSING kind"
    # repository_name() for root cell should be "@"
    assert repo_name == "@", f"Expected '@' for root cell repository_name(), got '{repo_name}'"


@buck_test(data_dir="test_native_rules_data")
async def test_existing_rules_returns_attributes(buck: Buck) -> None:
    """native.existing_rules() returns all explicitly-set attributes for each target."""
    result = await buck.build("//:existing_rules_check")
    output = result.get_build_report().output_for_target("//:existing_rules_check")
    content = output.read_text().strip()
    lines = content.splitlines()
    line_dict = {}
    for line in lines:
        if "=" in line:
            key, val = line.split("=", 1)
            line_dict[key] = val

    # Verify that existing_rules() returns actual attributes
    # The "original" target is write_list with items=["hello", "world"]
    assert line_dict.get("original_has_items") == "True", \
        f"Expected 'items' attribute in existing_rules(), got: {line_dict.get('original_has_items')}"
    assert line_dict.get("original_items") == "hello,world", \
        f"Expected items=['hello','world'], got: {line_dict.get('original_items')}"

    # Verify existing_rule() returns attributes for a single target
    assert line_dict.get("single_kind") == "filegroup", \
        f"Expected filegroup for single rule, got: {line_dict.get('single_kind')}"
    assert line_dict.get("single_has_srcs") == "True", \
        f"Expected 'srcs' attribute in existing_rule(), got: {line_dict.get('single_has_srcs')}"


@buck_test(data_dir="test_native_rules_data")
async def test_starlark_doc_extract_builds(buck: Buck) -> None:
    """starlark_doc_extract rule exists (Bazel 7+ feature detection for rules_python)."""
    result = await buck.build("//:doc_extract_test")
    output = result.get_build_report().output_for_target("//:doc_extract_test")
    # Stub creates an empty output file, similar to genquery
    assert output.exists()


@buck_test(data_dir="test_native_rules_data")
async def test_hasattr_native_starlark_doc_extract(buck: Buck) -> None:
    """hasattr(native, 'starlark_doc_extract') returns True (rules_python IS_BAZEL_7_OR_HIGHER)."""
    # defs.bzl contains: if not hasattr(native, "starlark_doc_extract"): fail(...)
    # If any target from this file builds, the check passed.
    result = await buck.build("//:doc_extract_test")
    output = result.get_build_report().output_for_target("//:doc_extract_test")
    assert output.exists()


@buck_test(data_dir="test_native_rules_data")
async def test_cc_toolchain_registers_target(buck: Buck) -> None:
    """cc_toolchain() registers a resolvable target (not a no-op)."""
    result = await buck.targets("//:test_cc_toolchain")
    assert "//:test_cc_toolchain" in result.stdout.replace("root//:", "//:")


@buck_test(data_dir="test_native_rules_data")
async def test_cc_toolchain_suite_registers_target(buck: Buck) -> None:
    """cc_toolchain_suite() registers a resolvable target (not a no-op)."""
    result = await buck.targets("//:test_cc_toolchain_suite")
    assert "//:test_cc_toolchain_suite" in result.stdout.replace("root//:", "//:")


@buck_test(data_dir="test_native_rules_data")
async def test_cc_import_registers_target(buck: Buck) -> None:
    """cc_import() registers a resolvable target for prebuilt libraries."""
    result = await buck.targets("//:test_cc_import")
    assert "//:test_cc_import" in result.stdout.replace("root//:", "//:")


@buck_test(data_dir="test_native_rules_data")
async def test_cc_command_line_generation(buck: Buck) -> None:
    """cc_common.get_tool_for_action() and get_memory_inefficient_command_line() work."""
    result = await buck.build("//:cc_command_line_test")
    output = result.get_build_report().output_for_target("//:cc_command_line_test")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    # Compiler tool should exist (e.g. gcc, clang, cl.exe)
    assert lines.get("compiler_path", "") != "", f"compiler_path should be non-empty, got: {lines}"
    # Compile command line should contain the source and output file
    assert lines.get("has_source_in_compile") == "True", f"compile cmdline should contain test.cc: {lines}"
    assert lines.get("has_output_in_compile") == "True", f"compile cmdline should contain test.o: {lines}"
    # Linker tool should exist
    assert lines.get("linker_path", "") != "", f"linker_path should be non-empty, got: {lines}"
    # Both should have non-zero length command lines
    assert int(lines.get("compile_cmdline_len", "0")) > 0, f"compile cmdline should be non-empty: {lines}"
    assert int(lines.get("link_cmdline_len", "0")) > 0, f"link cmdline should be non-empty: {lines}"
    # Link command should contain output file from create_link_variables(output_file=...)
    assert lines.get("has_output_in_link") == "True", f"link cmdline should contain my_binary: {lines}"


@buck_test(data_dir="test_native_rules_data")
async def test_java_common_module_available(buck: Buck) -> None:
    """java_common module is available as a global with expected methods."""
    result = await buck.build("//:java_common_test")
    output = result.get_build_report().output_for_target("//:java_common_test")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines.get("type") == "java_common", f"Expected java_common type, got: {lines}"
    assert lines.get("has_compile") == "True", f"java_common should have compile method: {lines}"
    assert lines.get("has_merge") == "True", f"java_common should have merge method: {lines}"
    assert lines.get("has_boot_class_path") == "True", f"java_common should have boot_class_path: {lines}"
    assert lines.get("java_info_type") == "JavaInfo", f"JavaInfo should be available: {lines}"
    assert lines.get("java_plugin_info_type") == "JavaPluginInfo", f"JavaPluginInfo should be available: {lines}"
    # JavaInfo is callable and compile()/merge() return proper instances
    assert lines.get("java_info_callable") == "True", f"JavaInfo should be callable: {lines}"
    assert lines.get("compile_returns_java_info") == "True", f"compile() should return JavaInfo: {lines}"
    assert lines.get("merge_returns_java_info") == "True", f"merge() should return JavaInfo: {lines}"
    assert lines.get("plugin_info_callable") == "True", f"JavaPluginInfo should be callable: {lines}"
    assert lines.get("has_runtime_info") == "True", f"java_common.JavaRuntimeInfo should exist: {lines}"
    assert lines.get("has_toolchain_info") == "True", f"java_common.JavaToolchainInfo should exist: {lines}"


@buck_test(data_dir="test_native_rules_data")
async def test_cc_shared_library_builds(buck: Buck) -> None:
    """cc_shared_library native rule can be parsed and analyzed."""
    await buck.build("//:my_shared_lib")


@buck_test(data_dir="test_native_rules_data")
async def test_environment_group_removed_in_bazel9(buck: Buck) -> None:
    """environment_group is a Bazel-9-removed rule; loading succeeds but
    analysis emits a Bazel-shaped removed-rule diagnostic. See Plan 27."""
    with pytest.raises(Exception) as exc_info:
        await buck.build("//:jdk_versions")
    msg = str(exc_info.value)
    assert "environment_group" in msg
    assert "removed in Bazel 9" in msg


@buck_test(data_dir="test_native_rules_data")
async def test_sh_binary_removed_without_load(buck: Buck) -> None:
    """`sh_binary(...)` without a load fails with the removed-rule
    diagnostic. The diagnostic includes the `@rules_shell` load hint so
    users see how to migrate. See Plan 27.2."""
    with pytest.raises(Exception) as exc_info:
        await buck.build("//sh_removed:noload_sh_binary")
    msg = str(exc_info.value)
    assert "sh_binary" in msg
    assert "removed in Bazel 9" in msg
    assert "@rules_shell" in msg


@buck_test(data_dir="test_native_rules_data")
async def test_cc_library_removed_without_load(buck: Buck) -> None:
    """`cc_library(...)` without a load fails with the removed-rule
    diagnostic. The diagnostic includes the `@rules_cc` load hint so
    users see how to migrate. See Plan 27.2."""
    with pytest.raises(Exception) as exc_info:
        await buck.build("//cc_removed:noload_cc_library")
    msg = str(exc_info.value)
    assert "cc_library" in msg
    assert "removed in Bazel 9" in msg
    assert "@rules_cc" in msg


@buck_test(data_dir="test_native_rules_data")
async def test_28_2_kuro_builtins_visible_in_external_bzl(buck: Buck) -> None:
    """Plan 28.2 acceptance: a public symbol exported from
    `@kuro_builtins//:exports.bzl` is visible in an external `.bzl` file
    without an explicit `load()`. This fixture's MODULE.bazel does not
    register a prelude, so the only injection path is the new
    `bazel_builtins_autoload` (auto-registers `@kuro_builtins` as a
    bundled cell + imports its public symbols into every env).
    See `thoughts/shared/research/2026-04-30-plan-28-1-builtins-loader-spike.md`.
    """
    result = await buck.build("//:kuro_builtins_probe_target")
    output = result.get_build_report().output_for_target(
        "//:kuro_builtins_probe_target"
    )
    assert output.read_text().strip() == "kuro-28-2-loader-ok"


@buck_test(data_dir="test_native_rules_data")
async def test_28_4_stage2_wrapper_passes_through(buck: Buck) -> None:
    """Plan 28.4 Stage 2: every Starlark rule impl is now called as
    `rule_implementation_wrapper(impl, ctx)`. Stage 1's wrapper is a
    no-op (`return implementation(raw_ctx)`), so providers produced
    by the rule must match what `impl(ctx)` would have returned
    directly. This fixture writes a sentinel string and asserts it
    reaches the output verbatim — broken if the wrapper drops or
    mutates the result. The unchanged @llvm-project//llvm:Demangle
    build is the broader regression net. See Plan 28.4 in
    `thoughts/shared/plans/kuro-bazel-subplans/28-builtins-module-architecture.md`.
    """
    result = await buck.build("//wrapper_proof:wrapper_probe_target")
    output = result.get_build_report().output_for_target(
        "//wrapper_proof:wrapper_probe_target"
    )
    assert output.read_text().strip() == "wrapped-via-rule-implementation-wrapper-noop"


@buck_test(data_dir="test_native_rules_data")
async def test_28_4_stage3_facade_in_call_path(buck: Buck) -> None:
    """Plan 28.4 Stage 3: the bundled `rule_implementation_wrapper`
    installs a Starlark facade around `raw_ctx` and migrates
    `target_platform_has_constraint` from Rust into the facade. Two
    invariants verified by the fixture rule:

      1. `ctx.kuro_facade_active == True` — the marker proves the
         wrapper actually built a struct facade and passed it to the
         user's impl. If the wrapper degenerated back to
         `implementation(raw_ctx)`, the marker is missing and the
         build fails.

      2. `ctx.target_platform_has_constraint(...)` is served by the
         Starlark closure in `_invoke_rule` (the Rust impl was deleted
         as part of this stage — Plan 28.7 single-owner discipline).
         The fixture pins a positive case (a host-matching OS label)
         and a negative case (a non-host OS label) to guard the
         migration's behaviour.

    See Plan 28.4 in
    `thoughts/shared/plans/kuro-bazel-subplans/28-builtins-module-architecture.md`.
    """
    result = await buck.build("//wrapper_proof:facade_proof_target")
    output = result.get_build_report().output_for_target(
        "//wrapper_proof:facade_proof_target"
    )
    assert output.read_text().strip() == "facade-proof-ok"


@buck_test(data_dir="test_native_rules_data")
async def test_28_4_stage4_aspect_facade_in_call_path(buck: Buck) -> None:
    """Plan 28.4 Stage 4: aspect impls now flow through
    `aspect_implementation_wrapper(impl, target, ctx)`. The bundled
    wrapper installs a Starlark facade around the aspect's `raw_ctx`,
    sets `ctx.kuro_facade_active = True` + `ctx.kuro_facade_kind =
    "aspect"`, and reuses the same `_kuro_target_platform_has_constraint`
    shim Stage 3 installed for rule contexts. The fixture aspect runs
    on the leaf target (via `attr.label_list(aspects = [...])` on the
    collector) and stuffs its observations into a provider; the
    collector's rule impl then asserts every invariant before writing
    its sentinel output.

    The previous Rust aspect-side `target_platform_has_constraint` was
    a stub returning `False` unconditionally — Stage 3 deleted it, so
    a meaningful answer here doubly proves the Starlark shim is wired
    in via the aspect facade and not via leftover Rust code.
    """
    result = await buck.build("//wrapper_proof:aspect_facade_collector_target")
    output = result.get_build_report().output_for_target(
        "//wrapper_proof:aspect_facade_collector_target"
    )
    assert output.read_text().strip() == "aspect-facade-proof-ok"


@buck_test(data_dir="test_native_rules_data")
async def test_28_4_stage5_subrule_facade_in_call_path(buck: Buck) -> None:
    """Plan 28.4 Stage 5: subrule impls now flow through
    `subrule_implementation_wrapper(impl, ctx, **kwargs)`. The wrapper
    `Value` is stashed in TLS by `RuleSpec::Impl::invoke` for the
    duration of the rule's eval and read by
    `kuro_interpreter_for_build::subrule::FrozenStarlarkSubruleCallable::invoke`.

    The fixture rule calls a subrule with a sentinel kwarg. Inside
    the subrule impl we assert:

      1. `ctx.kuro_facade_active == True` (facade in path);
      2. `ctx.kuro_facade_kind == "subrule"` (subrule wrapper, not
         the leaked rule wrapper);
      3. `ctx.target_platform_has_constraint(...)` answers correctly
         (Starlark shim works inside subrules too); and
      4. the sentinel kwarg reaches the impl verbatim, proving
         `_invoke_subrule(implementation, raw_ctx, **kwargs)`
         forwards kwargs without dropping or rewriting.
    """
    result = await buck.build("//wrapper_proof:subrule_facade_target")
    output = result.get_build_report().output_for_target(
        "//wrapper_proof:subrule_facade_target"
    )
    assert output.read_text().strip() == "subrule-facade-proof-ok"


@buck_test(data_dir="test_native_rules_data")
async def test_28_4_stage6_package_relative_label_starlark(buck: Buck) -> None:
    """Plan 28.4 Stage 6: `ctx.package_relative_label` migrated from
    Rust to Starlark. The Rust impl in
    `app/kuro_build_api/src/interpreter/rule_defs/context.rs` was
    deleted as part of this stage; the facade closure
    `_kuro_package_relative_label` in `@kuro_builtins//:exports.bzl`
    now serves the call.

    The fixture exercises every branch of the previous Rust impl
    (bare target, `:target`, absolute `//pkg:target`, fully-qualified
    `@cell//...`) and pins the canonical Label string output. A
    regression here means the Starlark migration diverged from the
    Rust impl's input/output contract.
    """
    result = await buck.build("//wrapper_proof:package_relative_label_proof_target")
    output = result.get_build_report().output_for_target(
        "//wrapper_proof:package_relative_label_proof_target"
    )
    assert output.read_text().strip() == "package-relative-label-proof-ok"


@buck_test(data_dir="test_native_rules_data")
async def test_28_4_stage7_tokenize_starlark(buck: Buck) -> None:
    """Plan 28.4 Stage 7: `ctx.tokenize` migrated from Rust to
    Starlark. The Rust impl + its `shell_tokenize` helper in
    `app/kuro_build_api/src/interpreter/rule_defs/context.rs` were
    deleted; `_kuro_tokenize` in `@kuro_builtins//:exports.bzl` now
    serves the call.

    The pre-existing `test_tokenize` covers the basic shapes
    (unquoted, single-quoted, double-quoted, empty, multi-whitespace)
    and continues to pass through the Starlark impl. This test pins
    the edge cases the Rust impl handled but the basic test does
    not exercise: backslash escapes inside and outside quotes, all
    four escapable double-quote chars (``\"``, ``\\``, ``$``, `` ` ``),
    non-escapable char after backslash inside quotes (literal `\\`
    survives, next char NOT consumed — Rust quirk preserved),
    trailing `\\` dropped, all five ASCII whitespace forms as
    separators (space, `\\t`, `\\n`, `\\f`, `\\r`).
    """
    result = await buck.build("//wrapper_proof:tokenize_proof_target")
    output = result.get_build_report().output_for_target(
        "//wrapper_proof:tokenize_proof_target"
    )
    assert output.read_text().strip() == "tokenize-proof-ok"


@buck_test(data_dir="test_native_rules_data")
async def test_28_4_stage8_coverage_instrumented_starlark(buck: Buck) -> None:
    """Plan 28.4 Stage 8: `ctx.coverage_instrumented` migrated from
    Rust to Starlark. Demonstrates the "global state hook" migration
    pattern: the per-build `--collect_code_coverage` flag is exposed
    via a kuro-internal Starlark global
    (`kuro_collect_code_coverage()`) registered in
    `app/kuro_interpreter_for_build/src/interpreter/functions/kuro_runtime.rs`,
    and `_kuro_coverage_instrumented` in
    `@kuro_builtins//:exports.bzl` reads it.

    The Rust impl ignored both `this` and `dep` and unconditionally
    returned the global flag. The migrated function preserves that:
    `ctx.coverage_instrumented()` and `ctx.coverage_instrumented(None)`
    must return the flag's default (`False`) for this build, since
    no `--collect_code_coverage` is passed.
    """
    result = await buck.build("//wrapper_proof:coverage_instrumented_proof_target")
    output = result.get_build_report().output_for_target(
        "//wrapper_proof:coverage_instrumented_proof_target"
    )
    assert output.read_text().strip() == "coverage-instrumented-proof-ok"


@buck_test(data_dir="test_native_rules_data")
async def test_28_4_stage9_var_starlark(buck: Buck) -> None:
    """Plan 28.4 Stage 9: `ctx.var` migrated from Rust to Starlark.
    The Rust `#[starlark(attribute)] fn var` in
    `app/kuro_build_api/src/interpreter/rule_defs/context.rs` was
    deleted; `_kuro_var` (sharing `_kuro_make_substitutions` with
    `_kuro_expand_make_variables`) in `@kuro_builtins//:exports.bzl`
    now serves the field.

    The fixture pins:
      - All 13 builtin keys (BINDIR through STACK_FRAME_UNLIMITED)
        present and string-typed.
      - BINDIR/GENDIR mirror `ctx.bin_dir.path` (the facade reads
        the same Rust-side `CtxDirRoot` value the deleted impl did).
      - WORKSPACE_ROOT mirrors `ctx.label.workspace_root`.
      - Pinned constant strings (ABI, ABI_GLIBC_VERSION, CC_FLAGS,
        STACK_FRAME_UNLIMITED) match the Rust impl byte-for-byte.
      - `ctx.var.items()` works (the impl returns an actual dict).
    """
    result = await buck.build("//wrapper_proof:var_proof_target")
    output = result.get_build_report().output_for_target(
        "//wrapper_proof:var_proof_target"
    )
    assert output.read_text().strip() == "var-proof-ok"


@buck_test(data_dir="test_native_rules_data")
async def test_28_4_stage9_expand_make_variables_starlark(buck: Buck) -> None:
    """Plan 28.4 Stage 9: `ctx.expand_make_variables` migrated from
    Rust to Starlark. The Rust `fn expand_make_variables` in
    `app/kuro_build_api/src/interpreter/rule_defs/context.rs` was
    deleted; `_kuro_expand_make_variables` in
    `@kuro_builtins//:exports.bzl` now serves the call.

    The fixture pins behavioural parity with the deleted impl:
      - User-provided `additional_substitutions` win over builtins.
      - Builtins resolve when not overridden.
      - Unresolved `$(VAR)` patterns survive verbatim.
      - Unbalanced `$(` (no closing `)`) survives verbatim and the
        scan continues past it.
      - Multiple substitutions in one string all expand.
      - Whitespace inside `$(...)` is stripped (Rust `.trim()`).
      - `None` for `additional_substitutions` is accepted.
    """
    result = await buck.build("//wrapper_proof:expand_make_variables_proof_target")
    output = result.get_build_report().output_for_target(
        "//wrapper_proof:expand_make_variables_proof_target"
    )
    assert output.read_text().strip() == "expand-make-variables-proof-ok"


@buck_test(data_dir="test_native_rules_data")
async def test_28_4_stage13_expand_location_starlark(buck: Buck) -> None:
    """Plan 28.4 Stage 13: `ctx.expand_location` migrated from Rust to
    Starlark. The ~330-LOC Rust impl (pool-building + parser) in
    `app/kuro_build_api/src/interpreter/rule_defs/context.rs` was
    replaced by a stub; `_kuro_expand_location` in
    `@kuro_builtins//:exports.bzl` now serves the call, backed by
    the `kuro_collect_location_pool` and `kuro_lookup_output_path`
    runtime hooks.

    The fixture pins behavioural parity with the deleted impl:
      - $(location :dep) resolves to the dep's first output path.
      - $(locations :dep) joins all output paths with " ".
      - $(execpath :dep) and $(rootpath :dep) resolve identically.
      - Unresolved $(location :missing) survives verbatim.
      - Plain strings with no $(...) pass through unchanged.
      - Multiple substitutions in one string all expand.
    """
    result = await buck.build("//wrapper_proof:expand_location_proof_target")
    output = result.get_build_report().output_for_target(
        "//wrapper_proof:expand_location_proof_target"
    )
    assert output.read_text().strip() == "expand-location-proof-ok"


@buck_test(data_dir="test_native_rules_data")
async def test_28_3_export_contract_hides_unlisted_symbols(buck: Buck) -> None:
    """Plan 28.3: only names in `exported_toplevels` reach the consuming
    env. Symbols defined at the top level of `exports.bzl` but NOT
    listed in the dict (private helpers, the rule_implementation_wrapper
    hook, etc.) must remain invisible to user `.bzl` files. Visibility
    control lives in the bundled exports.bzl, not in the interpreter."""
    with pytest.raises(Exception) as exc_info:
        await buck.targets("//export_contract_hidden:test_export_contract_hidden")
    msg = str(exc_info.value)
    # `rule_implementation_wrapper` is defined at the top of exports.bzl
    # but intentionally NOT in `exported_toplevels` (it's a Phase 28.4
    # internal hook, not a user-visible builtin). Referencing it from
    # an external .bzl must fail at parse time with "not found".
    assert "rule_implementation_wrapper" in msg
    assert "not found" in msg


@buck_test(data_dir="test_native_rules_data")
async def test_loaded_removed_rules_analyze_cleanly(buck: Buck) -> None:
    """Plan 27.5 readiness gate: a user `load()` shadows the BUILD-global
    removed-rule stub. Loaded sh_binary / sh_library / sh_test (from the
    fixture's :defs.bzl Starlark replacements) and cc_library (no-op
    Starlark stub) analyze without firing the removed-rule diagnostic.

    The full @rules_cc / @rules_shell parity is exercised by
    @llvm-project//llvm:Demangle and :Support — see
    memory/llvm_smoke_test.md."""
    # All four targets exist and are loaded via :defs.bzl in the fixture's
    # BUILD.bazel. Building all of them in one shot proves the load shadows
    # the BUILD-global stub for every removed rule family touched by Plan
    # 27.2.
    await buck.build(
        "//:hello_sh",
        "//:hello_sh_lib",
        "//:hello_sh_test",
        "//:shared_lib_dep",
    )


@buck_test(data_dir="test_native_rules_data")
async def test_attr_int_values_valid(buck: Buck) -> None:
    """attr.int(values=[...]) accepts valid integer values."""
    result = await buck.build("//:int_values_valid")
    output = result.get_build_report().output_for_target("//:int_values_valid")
    content = output.read_text().strip()
    assert "stamp=1" in content


@buck_test(data_dir="test_native_rules_data")
async def test_attr_int_values_default(buck: Buck) -> None:
    """attr.int(values=[...]) accepts the default value."""
    result = await buck.build("//:int_values_default")
    output = result.get_build_report().output_for_target("//:int_values_default")
    content = output.read_text().strip()
    assert "stamp=0" in content


@buck_test(data_dir="test_attr_int_values_data")
async def test_attr_int_values_invalid(buck: Buck) -> None:
    """attr.int(values=[...]) rejects invalid integer values."""
    from buck2.tests.e2e_util.api.buck_result import BuckException

    with pytest.raises(BuckException):
        await buck.build("//:int_values_invalid")


# ============================================================================
# allow_empty=False tests
# ============================================================================


@buck_test(data_dir="test_native_rules_data")
async def test_allow_empty_label_list_valid(buck: Buck) -> None:
    """attr.label_list(allow_empty=False) accepts non-empty lists."""
    result = await buck.build("//:nonempty_deps_valid")
    output = result.get_build_report().output_for_target("//:nonempty_deps_valid")
    content = output.read_text().strip()
    assert content != ""


@buck_test(data_dir="test_native_rules_data")
async def test_allow_empty_string_list_valid(buck: Buck) -> None:
    """attr.string_list(allow_empty=False) accepts non-empty lists."""
    result = await buck.build("//:nonempty_strings_valid")
    output = result.get_build_report().output_for_target("//:nonempty_strings_valid")
    content = output.read_text().strip()
    assert "hello" in content
    assert "world" in content


@buck_test(data_dir="test_allow_empty_data")
async def test_allow_empty_label_list_rejects_empty(buck: Buck) -> None:
    """attr.label_list(allow_empty=False) rejects empty lists."""
    from buck2.tests.e2e_util.api.buck_result import BuckException

    with pytest.raises(BuckException):
        await buck.build("//:empty_deps_invalid")


@buck_test(data_dir="test_allow_empty_data")
async def test_allow_empty_string_list_rejects_empty(buck: Buck) -> None:
    """attr.string_list(allow_empty=False) rejects empty lists."""
    from buck2.tests.e2e_util.api.buck_result import BuckException

    with pytest.raises(BuckException):
        await buck.build("//:empty_strings_invalid")


@buck_test(data_dir="test_native_rules_data")
async def test_rule_executable_true(buck: Buck) -> None:
    """rule(executable=True) makes ctx.outputs.executable available."""
    result = await buck.build("//:my_executable")
    output = result.get_build_report().output_for_target("//:my_executable")
    content = output.read_text().strip()
    assert "echo hello" in content


@buck_test(data_dir="test_native_rules_data")
async def test_rule_non_executable(buck: Buck) -> None:
    """A rule without executable=True still works normally."""
    result = await buck.build("//:my_non_executable")
    output = result.get_build_report().output_for_target("//:my_non_executable")
    content = output.read_text().strip()
    assert "not executable" in content


@buck_test(data_dir="test_native_rules_data")
async def test_rule_provides_valid(buck: Buck) -> None:
    """rule(provides=[MyInfo]) passes when implementation returns MyInfo."""
    result = await buck.build("//:provides_valid")
    output = result.get_build_report().output_for_target("//:provides_valid")
    content = output.read_text().strip()
    assert content == "ok"


@buck_test(data_dir="test_provides_missing_data")
async def test_rule_provides_missing_rejects(buck: Buck) -> None:
    """rule(provides=[MyInfo]) fails when implementation does NOT return MyInfo."""
    from buck2.tests.e2e_util.api.buck_result import BuckException

    with pytest.raises(BuckException):
        await buck.build("//:missing_provider")


@buck_test(data_dir="test_native_rules_data")
async def test_rule_initializer_prefix(buck: Buck) -> None:
    """rule(initializer=...) transforms message to add INIT: prefix."""
    result = await buck.build("//:initializer_prefix_test")
    output = result.get_build_report().output_for_target("//:initializer_prefix_test")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    # stamp=0 should pass through unchanged
    assert lines["stamp"] == "0"
    # The initializer should have added the "INIT:" prefix
    assert lines["message"] == "INIT:hello"


@buck_test(data_dir="test_native_rules_data")
async def test_rule_initializer_bool_to_int(buck: Buck) -> None:
    """rule(initializer=...) transforms stamp=True (bool) to stamp=1 (int)."""
    result = await buck.build("//:initializer_bool_to_int")
    output = result.get_build_report().output_for_target("//:initializer_bool_to_int")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    # The initializer should have converted True -> 1
    assert lines["stamp"] == "1"
    # The initializer should have added the "INIT:" prefix
    assert lines["message"] == "INIT:hello"


@buck_test(data_dir="test_native_rules_data")
async def test_build_config_defaults(buck: Buck) -> None:
    """ctx.configuration exposes stamp_binaries, coverage_enabled, test_env defaults."""
    result = await buck.build("//:build_config_defaults")
    output = result.get_build_report().output_for_target("//:build_config_defaults")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    # Defaults: stamp off, coverage off, empty test_env
    assert lines["stamp_binaries"] == "False"
    assert lines["coverage_enabled"] == "False"
    assert lines["test_env"] == "{}"


@buck_test(data_dir="test_native_rules_data")
async def test_build_config_stamp_flag(buck: Buck) -> None:
    """--stamp flag sets ctx.configuration.stamp_binaries to True."""
    result = await buck.build("--stamp", "//:build_config_defaults")
    output = result.get_build_report().output_for_target("//:build_config_defaults")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    assert lines["stamp_binaries"] == "True"


@buck_test(data_dir="test_native_rules_data")
async def test_build_config_test_env_flag(buck: Buck) -> None:
    """--test_env flag sets ctx.configuration.test_env."""
    result = await buck.build("--test_env", "MY_VAR=my_val", "//:build_config_defaults")
    output = result.get_build_report().output_for_target("//:build_config_defaults")
    content = output.read_text().strip()
    lines = dict(line.split("=", 1) for line in content.splitlines())
    assert "MY_VAR" in lines["test_env"]
    assert "my_val" in lines["test_env"]


@buck_test(data_dir="test_native_rules_data")
async def test_instrumented_files_info(buck: Buck) -> None:
    """coverage_common.instrumented_files_info() returns provider with depset fields."""
    result = await buck.build("//:instrumented_files_test")
    output = result.get_build_report().output_for_target("//:instrumented_files_test")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert "InstrumentedFilesInfo" in lines["type"]
    assert lines["has_instrumented_files"] == "True"
    assert lines["has_metadata_files"] == "True"


@buck_test(data_dir="test_native_rules_data")
async def test_instrumented_files_info_empty(buck: Buck) -> None:
    """coverage_common.instrumented_files_info() with no args returns empty depsets."""
    result = await buck.build("//:instrumented_files_empty_test")
    output = result.get_build_report().output_for_target("//:instrumented_files_empty_test")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert "InstrumentedFilesInfo" in lines["type"]
    assert lines["has_instrumented_files"] == "True"
    assert lines["has_metadata_files"] == "True"


@buck_test(data_dir="test_native_rules_data")
async def test_is_tool_configuration(buck: Buck) -> None:
    """ctx.configuration.is_tool_configuration() returns False for normal targets."""
    result = await buck.build("//:is_tool_configuration_test")
    output = result.get_build_report().output_for_target("//:is_tool_configuration_test")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    # Normal targets (not in exec configuration) should return False
    assert lines["is_tool"] == "False"
    assert lines["type"] == "bool"


@buck_test(data_dir="test_native_rules_data")
async def test_split_attr(buck: Buck) -> None:
    """ctx.split_attr wraps attribute values in single-entry config dicts."""
    result = await buck.build("//:split_attr_test")
    output = result.get_build_report().output_for_target("//:split_attr_test")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["has_split_attr"] == "True"
    assert lines["is_dict"] == "True"
    assert lines["has_default_key"] == "True"
    assert lines["value"] == "hello_split"


@buck_test(data_dir="test_native_rules_data")
async def test_resolve_command(buck: Buck) -> None:
    """ctx.resolve_command() returns a 3-tuple of (inputs, command, manifests)."""
    result = await buck.build("//:resolve_command_test")
    output = result.get_build_report().output_for_target("//:resolve_command_test")
    content = output.read_text().replace("\r\n", "\n").strip()
    assert content == "resolve_command_ok"


@buck_test(data_dir="test_native_rules_data")
async def test_new_file(buck: Buck) -> None:
    """ctx.new_file() creates a declared artifact that can be written to."""
    result = await buck.build("//:new_file_test")
    output = result.get_build_report().output_for_target("//:new_file_test")
    content = output.read_text().replace("\r\n", "\n").strip()
    assert content == "new_file_ok"


@buck_test(data_dir="test_native_rules_data")
async def test_java_toolchain_stubs(buck: Buck) -> None:
    """Java toolchain stubs provide expected attributes for rules_java compatibility."""
    result = await buck.build("//:java_toolchain_test")
    output = result.get_build_report().output_for_target("//:java_toolchain_test")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["has_java"] == "True"
    assert lines["source_version"] == "11"
    assert lines["target_version"] == "11"
    assert lines["has_java_runtime"] == "True"
    assert lines["has_bootclasspath"] == "True"
    assert lines["has_jvm_opt"] == "True"
    assert lines["worker_support"] == "True"
    assert lines["has_java_runtime_attr"] == "True"
    assert lines["has_java_home"] == "True"
    assert lines["has_java_exe"] == "True"
    assert lines["version"] == "11"


@buck_test(data_dir="test_native_rules_data")
async def test_constraint_providers_callable(buck: Buck) -> None:
    """ConstraintSettingInfo and ConstraintValueInfo are callable providers."""
    result = await buck.build("//:constraint_provider_test")
    output = result.get_build_report().output_for_target("//:constraint_provider_test")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["cs_callable"] == "True"
    assert lines["cs_has_label"] == "True"
    assert lines["cv_callable"] == "True"


@buck_test(data_dir="test_native_rules_data")
async def test_provider_callable(buck: Buck) -> None:
    """DebugPackageInfo and CcSharedLibraryInfo are callable providers, not None."""
    result = await buck.build("//:provider_callable_test")
    output = result.get_build_report().output_for_target("//:provider_callable_test")
    content = output.read_text().replace("\r\n", "\n")
    lines = dict(line.split("=", 1) for line in content.strip().split("\n") if "=" in line)

    assert lines["debug_is_not_none"] == "True"
    assert lines["debug_type"] == "DebugPackageInfo"
    assert lines["debug_instance_ok"] == "True"
    assert lines["debug_target_label"] == "True"
    assert lines["shared_is_not_none"] == "True"
    assert lines["shared_type"] == "CcSharedLibraryInfo"
    assert lines["shared_instance_ok"] == "True"


@buck_test(data_dir="test_native_rules_data")
async def test_write_file(buck: Buck) -> None:
    """ctx.actions.write_file() is a Bazel-compatible alias for ctx.actions.write()."""
    result = await buck.build("//:write_file_test")
    output = result.get_build_report().output_for_target("//:write_file_test")
    content = output.read_text().replace("\r\n", "\n").strip()
    assert content == "hello from write_file"


@buck_test(data_dir="test_native_rules_data")
async def test_write_file_executable(buck: Buck) -> None:
    """ctx.actions.write_file() supports is_executable positional arg."""
    result = await buck.build("//:write_file_executable_test")
    output = result.get_build_report().output_for_target("//:write_file_executable_test")
    content = output.read_text().replace("\r\n", "\n").strip()
    assert "echo write_file_exec" in content


@buck_test(data_dir="test_native_rules_data")
async def test_do_nothing_binds_outputs(buck: Buck) -> None:
    """ctx.actions.do_nothing() binds output artifacts so they can be built."""
    result = await buck.build("//:do_nothing_binds_test")
    output = result.get_build_report().output_for_target("//:do_nothing_binds_test")
    # do_nothing writes empty content; just verify the build succeeded
    assert output.exists()


@buck_test(data_dir="test_native_rules_data")
async def test_cc_toolchain_config_info(buck: Buck) -> None:
    """cc_common.create_cc_toolchain_config_info() creates a provider with accessible attributes."""
    result = await buck.build("//:cc_toolchain_config_info_test")
    output = result.get_build_report().output_for_target("//:cc_toolchain_config_info_test")
    content = output.read_text().strip()
    assert "cc_toolchain_config_info: ok" in content


@buck_test(data_dir="test_native_rules_data")
async def test_tokenize(buck: Buck) -> None:
    """ctx.tokenize() splits shell command strings."""
    result = await buck.build("//:tokenize_test")
    output = result.get_build_report().output_for_target("//:tokenize_test")
    content = output.read_text().strip()
    assert "tokenize: ok" in content


@buck_test(data_dir="test_native_rules_data")
async def test_files_to_run(buck: Buck) -> None:
    """DefaultInfo.files_to_run returns struct with executable and runfiles_manifest."""
    result = await buck.build("//:files_to_run_test")
    output = result.get_build_report().output_for_target("//:files_to_run_test")
    content = output.read_text().strip()
    assert "files_to_run: ok" in content


@buck_test(data_dir="test_native_rules_data")
async def test_actions_fail(buck: Buck) -> None:
    """ctx.actions.fail() raises an error during analysis."""
    with pytest.raises(Exception) as exc_info:
        await buck.build("//:actions_fail_test")
    assert "unsupported platform" in str(exc_info.value)


@buck_test(data_dir="test_native_rules_data")
async def test_merge_compilation_contexts(buck: Buck) -> None:
    """cc_common.merge_compilation_contexts() merges all include types and defines."""
    result = await buck.build("//:merge_compilation_contexts_test")
    output = result.get_build_report().output_for_target(
        "//:merge_compilation_contexts_test"
    )
    content = output.read_text().strip()
    assert "merge_compilation_contexts: ok" in content


@buck_test(data_dir="test_native_rules_data")
async def test_built_in_include_dirs(buck: Buck) -> None:
    """cc_toolchain.built_in_include_directories returns a list of strings."""
    result = await buck.build("//:built_in_include_dirs_test")
    output = result.get_build_report().output_for_target(
        "//:built_in_include_dirs_test"
    )
    content = output.read_text().strip()
    assert "built_in_include_dirs:" in content


@buck_test(data_dir="test_native_rules_data")
async def test_merge_cc_infos_full(buck: Buck) -> None:
    """merge_cc_infos preserves quote_includes, system_includes, framework_includes."""
    result = await buck.build("//:merge_cc_infos_full_test")
    output = result.get_build_report().output_for_target("//:merge_cc_infos_full_test")
    content = output.read_text().strip()
    assert "merge_cc_infos_full: ok" in content


@buck_test(data_dir="test_native_rules_data")
async def test_resolve_command_tools(buck: Buck) -> None:
    """ctx.resolve_command() returns proper tuple with tool inputs."""
    result = await buck.build("//:resolve_command_tools_test")
    output = result.get_build_report().output_for_target("//:resolve_command_tools_test")
    content = output.read_text().strip()
    assert "resolve_command_tools: ok" in content


@buck_test(data_dir="test_native_rules_data")
async def test_write_mnemonic(buck: Buck) -> None:
    """actions.write() accepts mnemonic and execution_requirements params (Bazel 9)."""
    result = await buck.build("//:write_mnemonic_test")
    output = result.get_build_report().output_for_target("//:write_mnemonic_test")
    content = output.read_text().strip()
    assert "write_mnemonic: ok" in content


@buck_test(data_dir="test_native_rules_data")
async def test_dir_ctx_files(buck: Buck) -> None:
    """dir(ctx.files) returns attribute names (Bazel compat)."""
    result = await buck.build("//:dir_ctx_files_test")
    output = result.get_build_report().output_for_target("//:dir_ctx_files_test")
    content = output.read_text().strip()
    attrs = content.split("\n")
    # Should include at least 'data' and 'srcs' from the rule definition
    assert "data" in attrs, f"Expected 'data' in dir(ctx.files), got: {attrs}"
    assert "srcs" in attrs, f"Expected 'srcs' in dir(ctx.files), got: {attrs}"


@buck_test(data_dir="test_native_rules_data")
async def test_package_specification_info(buck: Buck) -> None:
    """PackageSpecificationInfo is callable and returns an instance with packages."""
    result = await buck.build("//:psi_test")
    output = result.get_build_report().output_for_target("//:psi_test")
    content = output.read_text().strip()
    lines = content.split("\n")
    assert lines[0] == "type:PackageSpecificationInfo", f"Wrong type: {lines[0]}"
    assert "//foo" in lines[1], f"Expected //foo in packages: {lines[1]}"
    assert "//bar/..." in lines[1], f"Expected //bar/... in packages: {lines[1]}"


@buck_test(data_dir="test_native_rules_data")
async def test_symbolic_macro(buck: Buck) -> None:
    """macro() built-in creates callable symbolic macros (Bazel 8.0+)."""
    result = await buck.build("//:macro_greeting")
    output = result.get_build_report().output_for_target("//:macro_greeting")
    content = output.read_text().strip()
    assert "hello from macro" in content, f"Expected greeting in output: {content}"

