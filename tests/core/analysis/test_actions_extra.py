# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the above-listed
# licenses.

# pyre-strict

import json

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_actions_extra_data")
async def test_write_json_dict(buck: Buck) -> None:
    """ctx.actions.write_json writes a struct as JSON dict."""
    result = await buck.build("//:write_json_dict")
    output = result.get_build_report().output_for_target("//:write_json_dict")
    data = json.loads(output.read_text())
    assert data["key"] == "value", f"Unexpected JSON: {data!r}"
    assert data["num"] == 42, f"Unexpected JSON: {data!r}"


@buck_test(data_dir="test_actions_extra_data")
async def test_write_json_struct(buck: Buck) -> None:
    """ctx.actions.write_json serializes a struct to JSON."""
    result = await buck.build("//:write_json_struct")
    output = result.get_build_report().output_for_target("//:write_json_struct")
    data = json.loads(output.read_text())
    assert data["name"] == "slug"
    assert data["version"] == 1
    assert data["active"] is True


@buck_test(data_dir="test_actions_extra_data")
async def test_write_json_list(buck: Buck) -> None:
    """ctx.actions.write_json serializes a list to JSON."""
    result = await buck.build("//:write_json_list")
    output = result.get_build_report().output_for_target("//:write_json_list")
    data = json.loads(output.read_text())
    assert data == ["a", "b", "c"], f"Unexpected JSON: {data!r}"


@buck_test(data_dir="test_actions_extra_data")
async def test_copy_file(buck: Buck) -> None:
    """ctx.actions.copy_file copies a source file to a new output."""
    result = await buck.build("//:copy_source")
    output = result.get_build_report().output_for_target("//:copy_source")
    content = output.read_text().strip()
    assert "hello from source" in content, f"Unexpected content: {content!r}"
