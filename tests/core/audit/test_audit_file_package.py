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


@buck_test()
async def test_audit_file_package_simple(buck: Buck) -> None:
    """Test basic file-package mapping"""
    result = await buck.audit("file-package", "BUILD.bazel")
    assert ": root//" in result.stdout


@buck_test()
async def test_audit_file_package_json(buck: Buck) -> None:
    """Test file-package mapping with JSON output"""
    result = await buck.audit("file-package", "BUILD.bazel", "--json")

    data = json.loads(result.stdout)
    expected = {"BUILD.bazel": {"package": "root//"}}
    assert data == expected, f"Expected {expected}, got {data}"


@buck_test()
async def test_audit_file_package_newcell_json(buck: Buck) -> None:
    """Test file-package mapping for a file in 'newcell'"""
    # Assume 'newcell/BUILD.bazel' exists in the test workspace
    result = await buck.audit("file-package", "newcell/BUILD.bazel", "--json")

    data = json.loads(result.stdout)
    expected = {"newcell/BUILD.bazel": {"package": "newcell//"}}
    assert data == expected, f"Expected {expected}, got {data}"


@buck_test()
async def test_audit_file_package_multiple_paths_json(buck: Buck) -> None:
    """Test file-package mapping with multiple paths, including a file in 'newcell'"""
    result = await buck.audit(
        "file-package",
        "BUILD.bazel",
        "subdir/testfile",
        "newcell/BUILD.bazel",
        "--json",
    )

    data = json.loads(result.stdout)
    expected = {
        "BUILD.bazel": {"package": "root//"},
        "subdir/testfile": {"package": "root//subdir"},
        "newcell/BUILD.bazel": {"package": "newcell//"},
    }
    assert data == expected, f"Expected {expected}, got {data}"


@buck_test()
async def test_audit_file_package_with_errors_json(buck: Buck) -> None:
    """Test file-package mapping with a mix of valid and invalid paths"""
    result = await buck.audit(
        "file-package",
        "BUILD.bazel",
        "nonexistent/file.txt",
        "newcell/BUILD.bazel",
        "--json",
    )

    data = json.loads(result.stdout)
    expected = {
        "BUILD.bazel": {"package": "root//"},
        "newcell/BUILD.bazel": {"package": "newcell//"},
        "nonexistent/file.txt": {"error": "Error listing dir `nonexistent`"},
    }
    assert data == expected, f"Expected {expected}, got {data}"


@buck_test()
async def test_audit_file_package_with_errors_plain(buck: Buck) -> None:
    """Test file-package mapping with a mix of valid and invalid paths (plain text)"""
    result = await buck.audit(
        "file-package",
        "BUILD.bazel",
        "nonexistent/file.txt",
        "newcell/BUILD.bazel",
    )

    # Verify successful paths are in the output with correct format
    assert "BUILD.bazel: root//" in result.stdout
    assert "newcell/BUILD.bazel: newcell//" in result.stdout

    # Verify error path shows error message
    assert "nonexistent/file.txt: Error:" in result.stdout


@buck_test()
async def test_audit_file_package_absolute_path(buck: Buck) -> None:
    """Test file-package mapping with an absolute path"""
    abs_path = str(buck.cwd / "BUILD.bazel")
    result = await buck.audit("file-package", abs_path, "--json")

    data = json.loads(result.stdout)
    expected = {abs_path: {"package": "root//"}}
    assert data == expected, f"Expected {expected}, got {data}"
