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
