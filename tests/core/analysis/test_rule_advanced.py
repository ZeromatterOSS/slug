# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for advanced rule() features: provides, initializer, private attrs, executable, test."""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_rule_advanced_data")
async def test_provider_propagation(buck: Buck) -> None:
    """CompileInfo provider propagates through deps to link_rule."""
    result = await buck.build("//:linked")
    output = result.get_build_report().output_for_target("//:linked")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["objects"] == "2", f"Expected 2 objects, got {lines['objects']}"
    defines = set(lines["defines"].split(","))
    assert defines == {"DEBUG", "FEATURE_A", "FEATURE_B"}, f"Unexpected defines: {defines}"


@buck_test(data_dir="test_rule_advanced_data")
async def test_rule_initializer(buck: Buck) -> None:
    """Rule initializer transforms attribute values before rule instantiation."""
    result = await buck.build("//:processed_target")
    output = result.get_build_report().output_for_target("//:processed_target")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["prefix"] == "init_processed_target", f"Unexpected prefix: {lines['prefix']}"
    assert lines["value"] == "HELLO", f"Expected 'HELLO', got '{lines['value']}'"


@buck_test(data_dir="test_rule_advanced_data")
async def test_private_attrs(buck: Buck) -> None:
    """Private attributes (starting with _) have default values and aren't user-settable."""
    result = await buck.build("//:private_attrs_target")
    output = result.get_build_report().output_for_target("//:private_attrs_target")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["public_val"] == "user_value"
    assert lines["has_tool"] == "True"  # _tool has default "default_tool"


@buck_test(data_dir="test_rule_advanced_data")
async def test_executable_rule(buck: Buck) -> None:
    """Rule with executable=True builds successfully."""
    result = await buck.build("//:my_exe")
    output = result.get_build_report().output_for_target("//:my_exe")
    content = output.read_text()
    assert "echo hello" in content


@buck_test(data_dir="test_rule_advanced_data")
async def test_existing_rules_count(buck: Buck) -> None:
    """native.existing_rules() returns all rules defined so far in BUILD file."""
    result = await buck.build("//:rule_count_check")
    output = result.get_build_report().output_for_target("//:rule_count_check")
    content = output.read_text().strip()
    # 7 rules defined before count_rules() call
    count = int(content)
    assert count == 7, f"Expected 7 rules, got {count}"
