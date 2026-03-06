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


@buck_test(data_dir="test_providers_data")
async def test_user_defined_provider_fields(buck: Buck) -> None:
    """provider() creates providers with named fields accessible on instances."""
    result = await buck.build("//:read_provider")
    output = result.get_build_report().output_for_target("//:read_provider")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["message"] == "hello provider"
    assert lines["count"] == "42"


@buck_test(data_dir="test_providers_data")
async def test_provider_in_operator(buck: Buck) -> None:
    """The 'in' operator checks if a provider is present on a target."""
    result = await buck.build("//:check_provider")
    output = result.get_build_report().output_for_target("//:check_provider")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    # :my_rule_target provides MyInfo but not TagInfo
    assert lines["has_my_info"] == "True"
    assert lines["has_tag_info"] == "False"


@buck_test(data_dir="test_providers_data")
async def test_output_group_info(buck: Buck) -> None:
    """OutputGroupInfo returns multiple output groups alongside DefaultInfo."""
    result = await buck.build("//:multi_groups")
    # Default outputs are group_a.txt (from DefaultInfo(files=depset([out_a])))
    outputs = result.get_build_report().outputs_for_target("//:multi_groups")
    names = {p.name for p in outputs}
    assert "group_a.txt" in names


@buck_test(data_dir="test_providers_data")
async def test_provider_list_field(buck: Buck) -> None:
    """Provider fields can hold file lists."""
    result = await buck.build("//:flat_provider")
    output = result.get_build_report().output_for_target("//:flat_provider")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["name"] == "flat_provider"
    assert int(lines["src_count"]) == 2


@buck_test(data_dir="test_providers_data")
async def test_default_info_executable(buck: Buck) -> None:
    """DefaultInfo with executable= makes a target runnable."""
    result = await buck.build("//:my_executable")
    # Just verify it builds without errors
    build_report = result.get_build_report()
    assert build_report is not None
