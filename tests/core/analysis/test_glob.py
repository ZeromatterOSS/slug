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


@buck_test(data_dir="test_glob_data")
async def test_glob_all_txt(buck: Buck) -> None:
    """glob(["src/**/*.txt"]) finds all .txt files in src/."""
    result = await buck.build("//:all_txt")
    output = result.get_build_report().output_for_target("//:all_txt")
    names = output.read_text().strip().splitlines()
    assert "a.txt" in names, f"Expected a.txt in {names}"
    assert "b.txt" in names, f"Expected b.txt in {names}"
    assert "c.py" not in names, f"c.py should not match *.txt: {names}"


@buck_test(data_dir="test_glob_data")
async def test_glob_py_extension(buck: Buck) -> None:
    """glob(["src/**/*.py"]) finds only .py files."""
    result = await buck.build("//:all_py")
    output = result.get_build_report().output_for_target("//:all_py")
    names = output.read_text().strip().splitlines()
    assert "c.py" in names, f"Expected c.py in {names}"
    assert "a.txt" not in names, f"a.txt should not match *.py: {names}"
    assert "b.txt" not in names, f"b.txt should not match *.py: {names}"


@buck_test(data_dir="test_glob_data")
async def test_glob_with_exclude(buck: Buck) -> None:
    """glob with exclude= omits specified files."""
    result = await buck.build("//:txt_no_b")
    output = result.get_build_report().output_for_target("//:txt_no_b")
    names = output.read_text().strip().splitlines()
    assert "a.txt" in names, f"Expected a.txt in {names}"
    assert "b.txt" not in names, f"b.txt should be excluded: {names}"


@buck_test(data_dir="test_glob_data")
async def test_glob_multiple_patterns(buck: Buck) -> None:
    """glob with multiple patterns combines results."""
    result = await buck.build("//:multi_pattern")
    output = result.get_build_report().output_for_target("//:multi_pattern")
    names = output.read_text().strip().splitlines()
    # Should include .txt files from src/ and .json files from data/
    assert "a.txt" in names, f"Expected a.txt in {names}"
    assert "b.txt" in names, f"Expected b.txt in {names}"
    assert "x.json" in names, f"Expected x.json in {names}"
    assert "y.json" in names, f"Expected y.json in {names}"


@buck_test(data_dir="test_glob_data")
async def test_glob_no_match_returns_empty(buck: Buck) -> None:
    """glob() with no matching files returns empty list (rule still builds)."""
    result = await buck.build("//:no_match")
    output = result.get_build_report().output_for_target("//:no_match")
    content = output.read_text().strip()
    assert content == "", f"Expected empty output for no matches, got: {content!r}"


@buck_test(data_dir="test_glob_data")
async def test_glob_package_root(buck: Buck) -> None:
    """glob(["data/*.json"]) finds files in a subdirectory."""
    result = await buck.build("//:root_files")
    output = result.get_build_report().output_for_target("//:root_files")
    names = output.read_text().strip().splitlines()
    assert "x.json" in names, f"Expected x.json in {names}"
    assert "y.json" in names, f"Expected y.json in {names}"
