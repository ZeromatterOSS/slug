# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the above-listed
# licenses.

# pyre-strict

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_provider_init_data")
async def test_provider_init_transforms_value(buck: Buck) -> None:
    """provider(init=fn) calls init function to transform arguments."""
    result = await buck.build("//:init_basic")
    output = result.get_build_report().output_for_target("//:init_basic")
    content = output.read_text().strip()
    # Init function uppercases the string
    assert content == "HELLO", f"Expected 'HELLO' (uppercased via init), got: {content!r}"


@buck_test(data_dir="test_provider_init_data")
async def test_provider_raw_constructor_bypasses_init(buck: Buck) -> None:
    """The raw constructor (second element of tuple) bypasses the init function."""
    result = await buck.build("//:raw_bypass")
    output = result.get_build_report().output_for_target("//:raw_bypass")
    content = output.read_text().strip()
    # Raw constructor does NOT uppercase - bypasses init
    assert content == "hello", f"Expected 'hello' (raw, no init), got: {content!r}"


@buck_test(data_dir="test_provider_init_data")
async def test_provider_init_multiple_fields(buck: Buck) -> None:
    """provider(init=fn) can transform multiple fields in one init call."""
    result = await buck.build("//:multi_init")
    output = result.get_build_report().output_for_target("//:multi_init")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    # label_name should be stripped of whitespace
    assert lines["label_name"] == "my_target", f"Expected stripped name, got: {lines['label_name']!r}"
    # count should be incremented by init (5+1=6)
    assert lines["count"] == "6", f"Expected count=6 (init increments by 1), got: {lines['count']!r}"
    # display should combine both
    assert lines["display"] == "my_target (count=6)", f"Got: {lines['display']!r}"


@buck_test(data_dir="test_provider_init_data")
async def test_provider_init_with_files(buck: Buck) -> None:
    """provider(init=fn) works with file list arguments."""
    result = await buck.build("//:validated")
    output = result.get_build_report().output_for_target("//:validated")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert int(lines["file_count"]) == 1, f"Expected 1 src file, got: {lines['file_count']!r}"
    assert int(lines["header_count"]) == 1, f"Expected 1 header file, got: {lines['header_count']!r}"
