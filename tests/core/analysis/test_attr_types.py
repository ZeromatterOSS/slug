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


@buck_test(data_dir="test_attr_types_data")
async def test_string_dict_attr(buck: Buck) -> None:
    """attr.string_dict() creates a dict attribute accessible as ctx.attr.name[key]."""
    result = await buck.build("//:string_dict_target")
    output = result.get_build_report().output_for_target("//:string_dict_target")
    lines = dict(line.split("=", 1) for line in output.read_text().strip().splitlines())
    assert lines["key_a"] == "val_a"
    assert lines["key_b"] == "val_b"


@buck_test(data_dir="test_attr_types_data")
async def test_string_dict_iteration(buck: Buck) -> None:
    """attr.string_dict() values can be iterated to get sorted keys."""
    result = await buck.build("//:string_dict_iter")
    output = result.get_build_report().output_for_target("//:string_dict_iter")
    content = output.read_text().strip()
    keys = content.splitlines()
    assert "alpha" in keys
    assert "beta" in keys
    assert "gamma" in keys


@buck_test(data_dir="test_attr_types_data")
async def test_string_list_dict_attr(buck: Buck) -> None:
    """attr.string_list_dict() maps string keys to lists of strings."""
    result = await buck.build("//:string_list_dict_target")
    output = result.get_build_report().output_for_target("//:string_list_dict_target")
    content = output.read_text().strip()
    lines = content.splitlines()
    # unix key should map to ["-DUNIX", "-DPOSIX"]
    assert "unix:-DUNIX,-DPOSIX" in lines
    # win key should map to ["-DWIN32"]
    assert "win:-DWIN32" in lines


@buck_test(data_dir="test_attr_types_data")
async def test_label_keyed_string_dict_attr(buck: Buck) -> None:
    """attr.label_keyed_string_dict() maps label keys to string values."""
    result = await buck.build("//:label_keyed_dict_target")
    output = result.get_build_report().output_for_target("//:label_keyed_dict_target")
    content = output.read_text().strip()
    # The value associated with :dep_a should be "role_a"
    assert "dep_a.txt:role_a" in content
    assert "dep_b.txt:role_b" in content


@buck_test(data_dir="test_attr_types_data")
async def test_attr_output(buck: Buck) -> None:
    """attr.output() declares a single named output file."""
    result = await buck.build("//:output_attr_target")
    output = result.get_build_report().output_for_target("//:output_attr_target")
    content = output.read_text().strip()
    assert content == "output_written"


@buck_test(data_dir="test_attr_types_data")
async def test_attr_output_list(buck: Buck) -> None:
    """attr.output_list() declares multiple named output files."""
    result = await buck.build("//:output_list_target")
    outputs = result.get_build_report().outputs_for_target("//:output_list_target")
    by_name = {p.name: p for p in outputs}
    assert by_name["first.txt"].read_text().strip() == "first"
    assert by_name["second.txt"].read_text().strip() == "second"


@buck_test(data_dir="test_attr_types_data")
async def test_attr_int(buck: Buck) -> None:
    """attr.int() creates an integer attribute accessible as ctx.attr.name."""
    result = await buck.build("//:int_attr_target")
    output = result.get_build_report().output_for_target("//:int_attr_target")
    content = output.read_text().strip()
    assert content == "42"


@buck_test(data_dir="test_attr_types_data")
async def test_attr_bool(buck: Buck) -> None:
    """attr.bool() creates a boolean attribute."""
    result = await buck.build("//:bool_attr_true", "//:bool_attr_false")
    out_true = result.get_build_report().output_for_target("//:bool_attr_true")
    out_false = result.get_build_report().output_for_target("//:bool_attr_false")
    assert out_true.read_text().strip() == "True"
    assert out_false.read_text().strip() == "False"
