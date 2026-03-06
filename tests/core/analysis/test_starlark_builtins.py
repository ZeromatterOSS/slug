# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

import json

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_starlark_builtins_data")
async def test_struct_fields(buck: Buck) -> None:
    """struct() creates a record with named fields accessible via dot notation."""
    result = await buck.build("//:struct_fields")
    output = result.get_build_report().output_for_target("//:struct_fields")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["name"] == "test"
    assert lines["count"] == "42"
    assert lines["items"] == "a,b,c"


@buck_test(data_dir="test_starlark_builtins_data")
async def test_struct_hasattr_getattr(buck: Buck) -> None:
    """hasattr() and getattr() work on structs."""
    result = await buck.build("//:struct_hasattr")
    output = result.get_build_report().output_for_target("//:struct_hasattr")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["has_x"] == "True"
    assert lines["has_z"] == "False"
    assert lines["getattr_x"] == "1"
    assert lines["getattr_z"] == "99"  # default value


@buck_test(data_dir="test_starlark_builtins_data")
async def test_struct_nested(buck: Buck) -> None:
    """Nested structs can be accessed via chained dot notation."""
    result = await buck.build("//:struct_nested")
    output = result.get_build_report().output_for_target("//:struct_nested")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["outer_label"] == "outer"
    assert lines["inner_value"] == "inner_value"


@buck_test(data_dir="test_starlark_builtins_data")
async def test_json_encode(buck: Buck) -> None:
    """json.encode() serializes Starlark values to JSON strings."""
    result = await buck.build("//:json_encode")
    output = result.get_build_report().output_for_target("//:json_encode")
    content = output.read_text().strip()
    # Parse the output as JSON to verify correctness
    parsed = json.loads(content)
    assert parsed["name"] == "kuro"
    assert parsed["version"] == 9
    assert parsed["stable"] is True
    assert "fast" in parsed["tags"]
    assert "hermetic" in parsed["tags"]


@buck_test(data_dir="test_starlark_builtins_data")
async def test_json_decode(buck: Buck) -> None:
    """json.decode() parses JSON strings into Starlark dicts."""
    result = await buck.build("//:json_decode")
    output = result.get_build_report().output_for_target("//:json_decode")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["key"] == "hello"
    assert lines["num"] == "42"


@buck_test(data_dir="test_starlark_builtins_data")
async def test_type_and_dir(buck: Buck) -> None:
    """type() returns the type name and dir() lists struct fields."""
    result = await buck.build("//:type_dir")
    output = result.get_build_report().output_for_target("//:type_dir")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["type_string"] == "string"
    assert lines["type_int"] == "int"
    assert lines["type_list"] == "list"
    assert lines["type_dict"] == "dict"
    assert lines["type_bool"] == "bool"
    assert lines["type_none"] == "NoneType"
    # struct type name (may vary - just verify it's non-empty)
    assert lines["type_struct"] != ""
    assert lines["has_a_in_dir"] == "True"
