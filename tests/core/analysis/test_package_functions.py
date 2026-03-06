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


@buck_test(data_dir="test_package_functions_data")
async def test_package_name_root(buck: Buck) -> None:
    """package_name() returns empty string at root package."""
    result = await buck.build("//:package_name_test")
    output = result.get_build_report().output_for_target("//:package_name_test")
    assert output.read_text().strip() == ""


@buck_test(data_dir="test_package_functions_data")
async def test_subpackages(buck: Buck) -> None:
    """subpackages(include=["**"]) returns direct subpackage names."""
    result = await buck.build("//:subpackages_test")
    output = result.get_build_report().output_for_target("//:subpackages_test")
    content = output.read_text().strip()
    assert "sub1" in content
    assert "sub2" in content


@buck_test(data_dir="test_package_functions_data")
async def test_exports_files(buck: Buck) -> None:
    """exports_files() makes source files available as targets."""
    result = await buck.build("//:collect_exported")
    output = result.get_build_report().output_for_target("//:collect_exported")
    content = output.read_text().strip()
    assert "defs.bzl" in content


@buck_test(data_dir="test_package_functions_data")
async def test_repo_name(buck: Buck) -> None:
    """repo_name() returns the canonical repository name."""
    result = await buck.build("//:repo_name_test")
    output = result.get_build_report().output_for_target("//:repo_name_test")
    # For standalone module, returns the module name
    content = output.read_text().strip()
    assert isinstance(content, str)


@buck_test(data_dir="test_package_functions_data")
async def test_existing_rules(buck: Buck) -> None:
    """existing_rules() returns dict of rules defined so far in the BUILD file."""
    result = await buck.build("//:existing_rules_test")
    output = result.get_build_report().output_for_target("//:existing_rules_test")
    content = output.read_text().strip()
    # Rules defined before existing_rules_test should appear
    assert "package_name_test" in content
    assert "subpackages_test" in content
    # existing_rules_test itself should NOT be in the list (not yet defined)
    assert "existing_rules_test" not in content


@buck_test(data_dir="test_package_functions_data")
async def test_package_relative_label(buck: Buck) -> None:
    """package_relative_label(':name') returns absolute label for current package."""
    result = await buck.build("//:package_relative_label_test")
    output = result.get_build_report().output_for_target("//:package_relative_label_test")
    content = output.read_text().strip()
    # ":package_name_test" relative to root package "//" -> "//:package_name_test"
    assert content == "//:package_name_test"


@buck_test(data_dir="test_package_functions_data")
async def test_native_bazel_version(buck: Buck) -> None:
    """native.bazel_version returns a version >= 9.0.0 for Bazel compatibility."""
    result = await buck.build("//:bazel_version_test")
    output = result.get_build_report().output_for_target("//:bazel_version_test")
    version = output.read_text().strip()
    parts = version.split(".")
    assert len(parts) >= 2, f"Expected semver, got: {version!r}"
    assert int(parts[0]) >= 9, f"Expected major version >= 9, got: {version!r}"
