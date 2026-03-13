# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for BUILD-level global functions: existing_rules, package_name, glob, etc."""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_build_globals_data")
async def test_existing_rules_count(buck: Buck) -> None:
    """existing_rules() returns all rules defined so far in the BUILD file."""
    result = await buck.build("//:existing_rules_count")
    output = result.get_build_report().output_for_target("//:existing_rules_count")
    count = int(output.read_text().strip())
    # At least setup_target + 3 exports_files targets should exist
    assert count >= 4, f"Expected at least 4 rules, got {count}"


@buck_test(data_dir="test_build_globals_data")
async def test_existing_rule_kind(buck: Buck) -> None:
    """existing_rule() returns the 'kind' field for a defined rule."""
    result = await buck.build("//:existing_rule_kind")
    output = result.get_build_report().output_for_target("//:existing_rule_kind")
    content = output.read_text().strip()
    assert content == "genrule", f"Expected 'genrule', got '{content}'"


@buck_test(data_dir="test_build_globals_data")
async def test_existing_rule_missing(buck: Buck) -> None:
    """existing_rule() returns None for nonexistent targets."""
    result = await buck.build("//:existing_rule_missing")
    output = result.get_build_report().output_for_target("//:existing_rule_missing")
    content = output.read_text().strip()
    assert content == "NOT_FOUND", f"Expected 'NOT_FOUND', got '{content}'"


@buck_test(data_dir="test_build_globals_data")
async def test_package_name(buck: Buck) -> None:
    """native.package_name() returns the current package path."""
    result = await buck.build("//:package_name_test")
    output = result.get_build_report().output_for_target("//:package_name_test")
    content = output.read_text().strip()
    # Root package should be empty string
    assert content == "", f"Expected empty string for root package, got '{content}'"


@buck_test(data_dir="test_build_globals_data")
async def test_repository_name(buck: Buck) -> None:
    """native.repository_name() returns the repository name."""
    result = await buck.build("//:repo_name_test")
    output = result.get_build_report().output_for_target("//:repo_name_test")
    content = output.read_text().strip()
    # Root repo: Bazel returns "@" or "@@"
    assert content in ("@", "@@", ""), f"Expected '@' or '@@' or '', got '{content}'"


@buck_test(data_dir="test_build_globals_data")
async def test_glob_txt_count(buck: Buck) -> None:
    """native.glob(['*.txt']) matches text files in the package directory."""
    result = await buck.build("//:glob_txt_count")
    output = result.get_build_report().output_for_target("//:glob_txt_count")
    count = int(output.read_text().strip())
    # Should match: helper.txt, data1.txt, data2.txt (at least 3)
    assert count >= 3, f"Expected at least 3 .txt files, got {count}"


@buck_test(data_dir="test_build_globals_data")
async def test_glob_exclude(buck: Buck) -> None:
    """glob() with exclude parameter filters out specified files."""
    result = await buck.build("//:glob_exclude_count")
    output = result.get_build_report().output_for_target("//:glob_exclude_count")
    count = int(output.read_text().strip())
    # Should match data1.txt, data2.txt but NOT helper.txt (at least 2)
    assert count >= 2, f"Expected at least 2 files after exclude, got {count}"


@buck_test(data_dir="test_build_globals_data")
async def test_package_relative_label(buck: Buck) -> None:
    """package_relative_label() resolves label strings relative to current package."""
    result = await buck.build("//:relative_label_test")
    output = result.get_build_report().output_for_target("//:relative_label_test")
    content = output.read_text().strip()
    # Should contain "setup_target" in the resolved label
    assert "setup_target" in content, f"Expected 'setup_target' in '{content}'"


@buck_test(data_dir="test_build_globals_data")
async def test_filegroup_with_glob(buck: Buck) -> None:
    """filegroup with glob() as srcs collects matching files."""
    result = await buck.build("//:filegroup_glob_test")
    output = result.get_build_report().output_for_target("//:filegroup_glob_test")
    count = int(output.read_text().strip())
    # Should have at least 2 files (data1.txt, data2.txt)
    assert count >= 2, f"Expected at least 2 files in filegroup, got {count}"


@buck_test(data_dir="test_build_globals_data")
async def test_existing_rule_has_attrs(buck: Buck) -> None:
    """existing_rule() returns attribute data including 'srcs'."""
    result = await buck.build("//:existing_rule_has_srcs")
    output = result.get_build_report().output_for_target("//:existing_rule_has_srcs")
    content = output.read_text().strip()
    assert content == "True", f"Expected 'True', got '{content}'"


@buck_test(data_dir="test_build_globals_data")
async def test_module_name(buck: Buck) -> None:
    """native.module_name() returns the bzlmod module name."""
    result = await buck.build("//:module_name_test")
    output = result.get_build_report().output_for_target("//:module_name_test")
    content = output.read_text().strip()
    # Module name from MODULE.bazel should be "root"
    assert content in ("root", "None"), f"Expected 'root' or 'None', got '{content}'"


@buck_test(data_dir="test_build_globals_data")
async def test_repo_name_direct(buck: Buck) -> None:
    """repo_name() BUILD global returns the canonical repository name."""
    result = await buck.build("//:repo_name_direct_test")
    output = result.get_build_report().output_for_target("//:repo_name_direct_test")
    content = output.read_text().strip()
    # Should be empty or "@" for the root repo
    assert content in ("", "@", "@@"), f"Expected empty/@ for root repo, got '{content}'"
