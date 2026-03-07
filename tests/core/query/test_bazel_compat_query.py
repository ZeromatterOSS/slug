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

# All targets in this test use the "root" cell prefix (from MODULE.bazel: module(name = "root"))
# e.g. root//lib:core, root//app:main, root//:root_lib

_LIB_CORE = "root//lib:core"
_LIB_UTIL = "root//lib:util"
_APP_MAIN = "root//app:main"
_APP_SECONDARY = "root//app:secondary"
_ROOT_LIB = "root//:root_lib"
_ROOT_APP = "root//:root_app"


# ============================================================================
# deps() function
# ============================================================================


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_deps_direct(buck: Buck) -> None:
    """deps(//lib:util) includes //lib:util and //lib:core (transitive dep)."""
    result = await buck.uquery("deps(root//lib:util)")
    targets = set(result.stdout.strip().splitlines())
    assert _LIB_UTIL in targets, f"Expected {_LIB_UTIL} in {targets}"
    assert _LIB_CORE in targets, f"Expected {_LIB_CORE} in {targets}"


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_deps_transitive(buck: Buck) -> None:
    """deps(//app:main) includes app:main, lib:core, lib:util transitively."""
    result = await buck.uquery("deps(root//app:main)")
    targets = set(result.stdout.strip().splitlines())
    assert _APP_MAIN in targets
    assert _LIB_CORE in targets
    assert _LIB_UTIL in targets


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_deps_leaf_no_extra(buck: Buck) -> None:
    """deps(//lib:core) returns only itself (no deps)."""
    result = await buck.uquery("deps(root//lib:core)")
    targets = set(result.stdout.strip().splitlines())
    assert _LIB_CORE in targets
    assert _APP_MAIN not in targets, "app:main should not be in deps of lib:core"


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_deps_depth_1(buck: Buck) -> None:
    """deps(//lib:util, 1) returns only itself and direct deps."""
    result = await buck.uquery("deps(root//lib:util, 1)")
    targets = set(result.stdout.strip().splitlines())
    assert _LIB_UTIL in targets
    assert _LIB_CORE in targets


# ============================================================================
# rdeps() function
# ============================================================================


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_rdeps_universe(buck: Buck) -> None:
    """rdeps(//..., //lib:core) returns all targets that depend on //lib:core."""
    result = await buck.uquery("rdeps(root//..., root//lib:core)")
    targets = set(result.stdout.strip().splitlines())
    assert _LIB_CORE in targets  # rdeps includes the target itself
    assert _LIB_UTIL in targets  # util directly depends on core
    assert _APP_MAIN in targets  # main depends on core directly


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_rdeps_depth_1(buck: Buck) -> None:
    """rdeps(//..., //lib:core, 1) returns only direct reverse deps."""
    result = await buck.uquery("rdeps(root//..., root//lib:core, 1)")
    targets = set(result.stdout.strip().splitlines())
    assert _LIB_CORE in targets  # rdeps includes the target itself
    assert _LIB_UTIL in targets  # util directly depends on core


# ============================================================================
# kind() function
# ============================================================================


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_kind_lib_rule(buck: Buck) -> None:
    """kind("lib_rule", //...) returns only lib_rule targets."""
    result = await buck.uquery('kind("lib_rule", root//...)')
    targets = set(result.stdout.strip().splitlines())
    assert _LIB_CORE in targets
    assert _LIB_UTIL in targets
    assert _ROOT_LIB in targets
    # app_rule targets should not appear
    assert _APP_MAIN not in targets
    assert _APP_SECONDARY not in targets
    assert _ROOT_APP not in targets


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_kind_app_rule(buck: Buck) -> None:
    """kind("app_rule", //...) returns only app_rule targets."""
    result = await buck.uquery('kind("app_rule", root//...)')
    targets = set(result.stdout.strip().splitlines())
    assert _APP_MAIN in targets
    assert _APP_SECONDARY in targets
    assert _ROOT_APP in targets
    # lib_rule targets should not appear
    assert _LIB_CORE not in targets
    assert _LIB_UTIL not in targets


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_kind_regex(buck: Buck) -> None:
    """kind(".*_rule", //...) matches all custom rule targets."""
    result = await buck.uquery('kind(".*_rule", root//...)')
    targets = set(result.stdout.strip().splitlines())
    assert _LIB_CORE in targets
    assert _LIB_UTIL in targets
    assert _APP_MAIN in targets


# ============================================================================
# filter() function
# ============================================================================


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_filter_by_name_pattern(buck: Buck) -> None:
    """filter("core", //...) returns targets whose label contains 'core'."""
    result = await buck.uquery('filter("core", root//...)')
    targets = set(result.stdout.strip().splitlines())
    assert _LIB_CORE in targets
    # util, main, secondary should not match "core"
    assert _LIB_UTIL not in targets


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_filter_by_package_pattern(buck: Buck) -> None:
    """filter("root//app:.*", //...) returns only app package targets."""
    result = await buck.uquery('filter("root//app:.*", root//...)')
    targets = set(result.stdout.strip().splitlines())
    assert _APP_MAIN in targets
    assert _APP_SECONDARY in targets
    assert _LIB_CORE not in targets


# ============================================================================
# Set operations (+, -, ^)
# ============================================================================


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_set_union(buck: Buck) -> None:
    """//app/... + //lib/... returns all app and lib targets."""
    result = await buck.uquery("root//app/... + root//lib/...")
    targets = set(result.stdout.strip().splitlines())
    assert _APP_MAIN in targets
    assert _APP_SECONDARY in targets
    assert _LIB_CORE in targets
    assert _LIB_UTIL in targets
    assert _ROOT_APP not in targets


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_set_subtraction(buck: Buck) -> None:
    """//... - //app/... returns all targets except app ones."""
    result = await buck.uquery("root//... - root//app/...")
    targets = set(result.stdout.strip().splitlines())
    assert _LIB_CORE in targets
    assert _LIB_UTIL in targets
    assert _ROOT_LIB in targets
    assert _APP_MAIN not in targets
    assert _APP_SECONDARY not in targets


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_set_intersection(buck: Buck) -> None:
    """deps(//app:main) ^ deps(//app:secondary) returns common deps."""
    result = await buck.uquery(
        "deps(root//app:main) ^ deps(root//app:secondary)"
    )
    targets = set(result.stdout.strip().splitlines())
    # Both app:main and app:secondary depend on lib:util (which depends on lib:core)
    assert _LIB_UTIL in targets
    assert _LIB_CORE in targets
    # app:main itself should NOT be in the intersection
    assert _APP_MAIN not in targets


# ============================================================================
# allpaths() / somepath()
# ============================================================================


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_allpaths(buck: Buck) -> None:
    """allpaths(//app:main, //lib:core) returns all nodes on paths from main to core."""
    result = await buck.uquery("allpaths(root//app:main, root//lib:core)")
    targets = set(result.stdout.strip().splitlines())
    assert _APP_MAIN in targets
    assert _LIB_CORE in targets
    # Both direct and indirect paths should be included
    assert _LIB_UTIL in targets  # main -> util -> core is a path


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_somepath(buck: Buck) -> None:
    """somepath(//app:main, //lib:core) returns at least one path."""
    result = await buck.uquery("somepath(root//app:main, root//lib:core)")
    targets = set(result.stdout.strip().splitlines())
    assert _APP_MAIN in targets
    assert _LIB_CORE in targets
    # At least two nodes on the path
    assert len(targets) >= 2


# ============================================================================
# attr() function
# ============================================================================


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_attr_string_value(buck: Buck) -> None:
    """attr(app_name, main_application, //...) returns targets with matching attr value."""
    result = await buck.uquery('attr("app_name", "main_application", root//...)')
    targets = set(result.stdout.strip().splitlines())
    assert _APP_MAIN in targets
    assert _APP_SECONDARY not in targets


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_attr_regex(buck: Buck) -> None:
    """attr(app_name, ".*_application", //...) matches multiple targets by regex."""
    result = await buck.uquery('attr("app_name", ".*_application", root//...)')
    targets = set(result.stdout.strip().splitlines())
    assert _APP_MAIN in targets
    assert _APP_SECONDARY in targets
    assert _ROOT_APP in targets


# ============================================================================
# set() expression
# ============================================================================


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_set_expression(buck: Buck) -> None:
    """set(//lib:core //lib:util) returns exactly those two targets."""
    result = await buck.uquery("set(root//lib:core root//lib:util)")
    targets = set(result.stdout.strip().splitlines())
    assert targets == {_LIB_CORE, _LIB_UTIL}, f"Expected only 2 targets, got: {targets}"


# ============================================================================
# Target pattern //...
# ============================================================================


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_all_targets_pattern(buck: Buck) -> None:
    """//... enumerates all targets in the project."""
    result = await buck.uquery("root//...")
    targets = set(result.stdout.strip().splitlines())
    expected = {_ROOT_LIB, _ROOT_APP, _LIB_CORE, _LIB_UTIL, _APP_MAIN, _APP_SECONDARY}
    assert expected.issubset(targets), f"Missing targets. Expected {expected}, got {targets}"


# ============================================================================
# Output formats
# ============================================================================


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_output_json_format(buck: Buck) -> None:
    """--output=json returns a valid JSON array of target labels."""
    result = await buck.uquery("root//lib/...", "--output=json")
    data = json.loads(result.stdout)
    assert isinstance(data, list), f"Expected JSON array, got: {type(data)}"
    assert _LIB_CORE in data, f"Expected {_LIB_CORE} in JSON: {data}"
    assert _LIB_UTIL in data, f"Expected {_LIB_UTIL} in JSON: {data}"


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_output_build_format(buck: Buck) -> None:
    """--output=build returns BUILD-syntax rule definitions."""
    result = await buck.uquery("root//lib:core", "--output=build")
    content = result.stdout
    # Should contain the rule call with the target name
    assert "lib_rule" in content, f"Expected rule name in build output: {content!r}"
    assert '"core"' in content or "'core'" in content, \
        f"Expected target name 'core' in build output: {content!r}"


@buck_test(data_dir="test_bazel_compat_query_data")
async def test_output_default_label_format(buck: Buck) -> None:
    """Default output (--output=label) returns one label per line."""
    result = await buck.uquery("root//lib/...")
    lines = result.stdout.strip().splitlines()
    assert len(lines) == 2, f"Expected 2 targets, got: {lines}"
    assert _LIB_CORE in lines
    assert _LIB_UTIL in lines
