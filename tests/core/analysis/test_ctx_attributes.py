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


@buck_test(data_dir="test_ctx_attributes_data")
async def test_ctx_bin_dir_path(buck: Buck) -> None:
    """ctx.bin_dir.path returns a path rooted under buck-out."""
    result = await buck.build("//:bin_dir")
    output = result.get_build_report().output_for_target("//:bin_dir")
    path = output.read_text().strip()
    assert path.startswith("buck-out"), f"Expected buck-out prefix, got: {path!r}"
    assert "gen" in path, f"Expected 'gen' in path, got: {path!r}"


@buck_test(data_dir="test_ctx_attributes_data")
async def test_ctx_genfiles_dir_path(buck: Buck) -> None:
    """ctx.genfiles_dir.path returns a path rooted under buck-out (same as bin_dir in Kuro)."""
    result = await buck.build("//:genfiles_dir")
    output = result.get_build_report().output_for_target("//:genfiles_dir")
    path = output.read_text().strip()
    assert path.startswith("buck-out"), f"Expected buck-out prefix, got: {path!r}"


@buck_test(data_dir="test_ctx_attributes_data")
async def test_ctx_features_enabled(buck: Buck) -> None:
    """ctx.features returns features without the '-' prefix."""
    result = await buck.build("//:features_enabled")
    output = result.get_build_report().output_for_target("//:features_enabled")
    content = output.read_text().strip()
    features = content.splitlines() if content else []
    # Should include 'fast' and 'opt', but NOT '-debug' (which goes to disabled_features)
    assert "fast" in features
    assert "opt" in features
    assert "-debug" not in features
    assert "debug" not in features


@buck_test(data_dir="test_ctx_attributes_data")
async def test_ctx_disabled_features(buck: Buck) -> None:
    """ctx.disabled_features returns features that had a '-' prefix."""
    result = await buck.build("//:features_disabled")
    output = result.get_build_report().output_for_target("//:features_disabled")
    content = output.read_text().strip()
    features = content.splitlines() if content else []
    # Should include 'debug' and 'slow' (stripped of '-'), but NOT 'fast' or 'opt'
    assert "debug" in features
    assert "slow" in features
    assert "fast" not in features
    assert "opt" not in features


@buck_test(data_dir="test_ctx_attributes_data")
async def test_ctx_workspace_name_root(buck: Buck) -> None:
    """ctx.workspace_name returns '_main' for root cell targets."""
    result = await buck.build("//:workspace_name")
    output = result.get_build_report().output_for_target("//:workspace_name")
    name = output.read_text().strip()
    assert name == "_main", f"Expected '_main', got: {name!r}"


@buck_test(data_dir="test_ctx_attributes_data")
async def test_ctx_label_attrs(buck: Buck) -> None:
    """ctx.label has .package, .name, and .workspace_name attributes."""
    result = await buck.build("//:label_info")
    output = result.get_build_report().output_for_target("//:label_info")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["package"] == "", f"Root package should be empty, got: {lines['package']!r}"
    assert lines["name"] == "label_info"
    # In Bazel with bzlmod, Label.workspace_name returns "" for the main workspace
    assert lines["workspace"] == "", f"Root workspace should be empty, got: {lines['workspace']!r}"


@buck_test(data_dir="test_ctx_attributes_data")
async def test_ctx_expand_make_variables(buck: Buck) -> None:
    """ctx.expand_make_variables expands $(BINDIR), $(GENDIR), and custom vars."""
    result = await buck.build("//:make_vars")
    output = result.get_build_report().output_for_target("//:make_vars")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["bindir_starts_with_buck_out"] == "True"
    assert lines["gendir_starts_with_buck_out"] == "True"
    assert lines["custom"] == "custom_value"


@buck_test(data_dir="test_ctx_attributes_data")
async def test_ctx_actions_symlink(buck: Buck) -> None:
    """ctx.actions.symlink creates a file that resolves to the target's content."""
    result = await buck.build("//:file_symlink")
    output = result.get_build_report().output_for_target("//:file_symlink")
    content = output.read_text().strip()
    assert content == "symlink_source_content"


@buck_test(data_dir="test_ctx_attributes_data")
async def test_ctx_actions_expand_template(buck: Buck) -> None:
    """ctx.actions.expand_template substitutes placeholders in a template file."""
    result = await buck.build("//:expand_template")
    output = result.get_build_report().output_for_target("//:expand_template")
    content = output.read_text().strip()
    assert content == "Hello Kuro version 9.0.0!"


@buck_test(data_dir="test_ctx_attributes_data")
async def test_ctx_actions_do_nothing(buck: Buck) -> None:
    """ctx.actions.do_nothing() runs without error and doesn't prevent other actions."""
    result = await buck.build("//:do_nothing")
    output = result.get_build_report().output_for_target("//:do_nothing")
    content = output.read_text().strip()
    assert content == "do_nothing_ran"


@buck_test(data_dir="test_ctx_attributes_data")
async def test_ctx_resolve_tools(buck: Buck) -> None:
    """ctx.resolve_tools() collects file objects from tool deps."""
    result = await buck.build("//:resolve_tools")
    output = result.get_build_report().output_for_target("//:resolve_tools")
    content = output.read_text().strip()
    tool_names = content.splitlines() if content else []
    assert "tool_artifact.txt" in tool_names
