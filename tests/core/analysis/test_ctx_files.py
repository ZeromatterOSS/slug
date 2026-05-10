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


@buck_test(data_dir="test_ctx_files_data")
async def test_ctx_files_deps_from_rule(buck: Buck) -> None:
    """ctx.files.deps extracts a flat list of File objects from dep DefaultInfo."""
    result = await buck.build("//:files_from_lib")
    output = result.get_build_report().output_for_target("//:files_from_lib")

    content = output.read_text().strip().splitlines()
    assert set(content) == {"lib_a.txt", "lib_b.txt"}


@buck_test(data_dir="test_ctx_files_data")
async def test_ctx_files_deps_multiple(buck: Buck) -> None:
    """ctx.files.deps collects files from multiple deps."""
    result = await buck.build("//:files_from_multiple")
    output = result.get_build_report().output_for_target("//:files_from_multiple")

    content = output.read_text().strip().splitlines()
    assert set(content) == {"lib_a.txt", "lib_b.txt", "single.txt"}


@buck_test(data_dir="test_ctx_files_data")
async def test_ctx_files_deps_from_source(buck: Buck) -> None:
    """ctx.files.deps can collect source files as well."""
    result = await buck.build("//:files_from_source")
    output = result.get_build_report().output_for_target("//:files_from_source")

    content = output.read_text().strip().splitlines()
    assert set(content) == {"defs.bzl", "MODULE.bazel"}


@buck_test(data_dir="test_ctx_files_data")
async def test_ctx_attr_allow_files_source_entries_are_targets(buck: Buck) -> None:
    """Bazel label attrs expose source-file entries as Targets in ctx.attr."""
    result = await buck.build("//:ctx_attr_source_targets")
    output = result.get_build_report().output_for_target("//:ctx_attr_source_targets")

    lines = dict(
        line.split("=", 1) for line in output.read_text().strip().splitlines()
    )
    assert lines["attr_types"] == "Target,Target"
    assert set(lines["attr_labels"].split(",")) == {"defs.bzl", "single_dep"}
    assert lines["depset_count"] == "2"
    assert set(lines["default_files"].split(",")) == {"defs.bzl", "single.txt"}
    assert set(lines["projected_files"].split(",")) == {"defs.bzl", "single.txt"}


@buck_test(data_dir="test_ctx_files_data")
async def test_ctx_file_dep_single_file(buck: Buck) -> None:
    """ctx.file.dep gets a single File object from a dep with one output."""
    result = await buck.build("//:single_from_dep")
    output = result.get_build_report().output_for_target("//:single_from_dep")

    content = output.read_text().strip()
    assert content == "single.txt"


@buck_test(data_dir="test_ctx_files_data")
async def test_ctx_file_dep_source_file(buck: Buck) -> None:
    """ctx.file.dep works with direct source file labels."""
    result = await buck.build("//:single_from_source")
    output = result.get_build_report().output_for_target("//:single_from_source")

    content = output.read_text().strip()
    assert content == "defs.bzl"


@buck_test(data_dir="test_ctx_files_data")
async def test_ctx_expand_location(buck: Buck) -> None:
    """ctx.expand_location resolves $(location :target) to the output file path."""
    result = await buck.build("//:expand_location")
    output = result.get_build_report().output_for_target("//:expand_location")

    content = output.read_text().strip()
    # The expanded path should point to the output of :single_dep (single.txt)
    assert content.endswith("single.txt")
    assert "buck-out" in content


@buck_test(data_dir="test_ctx_files_data")
async def test_ctx_expand_rootpath(buck: Buck) -> None:
    """ctx.expand_location resolves $(rootpath :target) to the output file path."""
    result = await buck.build("//:expand_rootpath")
    output = result.get_build_report().output_for_target("//:expand_rootpath")

    content = output.read_text().strip()
    assert content.endswith("single.txt"), f"Expected path ending with single.txt, got: {content!r}"


@buck_test(data_dir="test_ctx_files_data")
async def test_ctx_expand_rlocationpath(buck: Buck) -> None:
    """ctx.expand_location resolves $(rlocationpath :target) to the output file path."""
    result = await buck.build("//:expand_rlocationpath")
    output = result.get_build_report().output_for_target("//:expand_rlocationpath")

    content = output.read_text().strip()
    assert content.endswith("single.txt"), f"Expected path ending with single.txt, got: {content!r}"
