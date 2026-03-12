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


@buck_test(data_dir="test_label_type_data")
async def test_label_name_attribute(buck: Buck) -> None:
    """Label.name returns the target name component."""
    result = await buck.build("//:label_attrs")
    output = result.get_build_report().output_for_target("//:label_attrs")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["name"] == "my_target", f"Expected 'my_target', got: {lines['name']!r}"


@buck_test(data_dir="test_label_type_data")
async def test_label_package_attribute(buck: Buck) -> None:
    """Label.package returns the package path (without leading //)."""
    result = await buck.build("//:label_attrs")
    output = result.get_build_report().output_for_target("//:label_attrs")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["package"] == "pkg/sub", f"Expected 'pkg/sub', got: {lines['package']!r}"


@buck_test(data_dir="test_label_type_data")
async def test_label_workspace_name_for_main(buck: Buck) -> None:
    """Label.workspace_name returns '' for main workspace labels."""
    result = await buck.build("//:label_attrs")
    output = result.get_build_report().output_for_target("//:label_attrs")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["workspace_name"] == "", f"Expected '' for main workspace, got: {lines['workspace_name']!r}"


@buck_test(data_dir="test_label_type_data")
async def test_label_root_package(buck: Buck) -> None:
    """Label('//:target').package == '' for root package."""
    result = await buck.build("//:label_root")
    output = result.get_build_report().output_for_target("//:label_root")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["name"] == "root_target", f"Expected 'root_target', got: {lines['name']!r}"
    assert lines["package"] == "", f"Expected '' for root package, got: {lines['package']!r}"


@buck_test(data_dir="test_label_type_data")
async def test_label_external_repo(buck: Buck) -> None:
    """Label('@repo//pkg:target') parses external repo correctly."""
    result = await buck.build("//:label_external")
    output = result.get_build_report().output_for_target("//:label_external")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["name"] == "my_lib", f"Expected 'my_lib', got: {lines['name']!r}"
    assert lines["package"] == "some/path", f"Expected 'some/path', got: {lines['package']!r}"
    assert lines["workspace_name"] == "my_repo", f"Expected 'my_repo', got: {lines['workspace_name']!r}"


@buck_test(data_dir="test_label_type_data")
async def test_label_relative(buck: Buck) -> None:
    """Label.relative(':other') resolves to same-package label."""
    result = await buck.build("//:label_relative")
    output = result.get_build_report().output_for_target("//:label_relative")
    content = output.read_text().strip()
    assert "other_target" in content, f"Expected 'other_target' in result, got: {content!r}"
    assert "pkg" in content, f"Expected package 'pkg' in result, got: {content!r}"


@buck_test(data_dir="test_label_type_data")
async def test_label_equality(buck: Buck) -> None:
    """Labels are equal iff they refer to the same target."""
    result = await buck.build("//:label_eq")
    output = result.get_build_report().output_for_target("//:label_eq")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["same_eq"] == "True", f"Same labels should be equal, got: {lines['same_eq']!r}"
    assert lines["diff_eq"] == "False", f"Different labels should not be equal, got: {lines['diff_eq']!r}"


@buck_test(data_dir="test_label_type_data")
async def test_label_str_conversion(buck: Buck) -> None:
    """str(Label()) returns the full label string."""
    result = await buck.build("//:label_str")
    output = result.get_build_report().output_for_target("//:label_str")
    content = output.read_text().strip()
    assert "my/package" in content, f"Expected package in str(label), got: {content!r}"
    assert "my_target" in content, f"Expected target in str(label), got: {content!r}"


@buck_test(data_dir="test_label_type_data")
async def test_label_same_package_label(buck: Buck) -> None:
    """Label.same_package_label() creates a label in the same package."""
    result = await buck.build("//:label_same_pkg")
    output = result.get_build_report().output_for_target("//:label_same_pkg")
    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["name"] == "sibling_target", f"Expected 'sibling_target', got: {lines['name']!r}"
    assert lines["package"] == "my/pkg", f"Expected 'my/pkg', got: {lines['package']!r}"
