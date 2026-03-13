# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

"""Tests for select() with config_setting: values, define_values, and list concatenation."""

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test(data_dir="test_select_config_setting_data")
async def test_select_default_compilation_mode(buck: Buck) -> None:
    """select() with config_setting(values=compilation_mode) defaults to fastbuild."""
    result = await buck.build("//:select_compilation_mode")
    output = result.get_build_report().output_for_target("//:select_compilation_mode")
    content = output.read_text().strip()
    assert content == "fastbuild", f"Expected 'fastbuild', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_select_opt_compilation_mode(buck: Buck) -> None:
    """select() matches config_setting(values={compilation_mode: opt}) with --compilation_mode=opt."""
    result = await buck.build("//:select_compilation_mode", "--compilation_mode=opt")
    output = result.get_build_report().output_for_target("//:select_compilation_mode")
    content = output.read_text().strip()
    assert content == "optimized", f"Expected 'optimized', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_select_dbg_compilation_mode(buck: Buck) -> None:
    """select() matches config_setting(values={compilation_mode: dbg}) with --compilation_mode=dbg."""
    result = await buck.build("//:select_compilation_mode", "--compilation_mode=dbg")
    output = result.get_build_report().output_for_target("//:select_compilation_mode")
    content = output.read_text().strip()
    assert content == "debug", f"Expected 'debug', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_select_define_default(buck: Buck) -> None:
    """select() with config_setting(define_values) defaults when no --define is set."""
    result = await buck.build("//:select_define")
    output = result.get_build_report().output_for_target("//:select_define")
    content = output.read_text().strip()
    assert content == "feature_x_default", f"Expected 'feature_x_default', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_select_define_enabled(buck: Buck) -> None:
    """select() matches config_setting(define_values={FEATURE_X: 1}) with --define FEATURE_X=1."""
    result = await buck.build("//:select_define", "--define", "FEATURE_X=1")
    output = result.get_build_report().output_for_target("//:select_define")
    content = output.read_text().strip()
    assert content == "feature_x_enabled", f"Expected 'feature_x_enabled', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_select_define_disabled(buck: Buck) -> None:
    """select() matches config_setting(define_values={FEATURE_X: 0}) with --define FEATURE_X=0."""
    result = await buck.build("//:select_define", "--define", "FEATURE_X=0")
    output = result.get_build_report().output_for_target("//:select_define")
    content = output.read_text().strip()
    assert content == "feature_x_disabled", f"Expected 'feature_x_disabled', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_select_list_concat(buck: Buck) -> None:
    """List concatenation: ['always'] + select({...}) produces combined list."""
    result = await buck.build("//:select_list_concat")
    output = result.get_build_report().output_for_target("//:select_list_concat")
    content = output.read_text().strip()
    assert content == "always,default_item", f"Expected 'always,default_item', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_select_list_concat_with_opt(buck: Buck) -> None:
    """List concatenation with --compilation_mode=opt."""
    result = await buck.build("//:select_list_concat", "--compilation_mode=opt")
    output = result.get_build_report().output_for_target("//:select_list_concat")
    content = output.read_text().strip()
    assert content == "always,opt_item", f"Expected 'always,opt_item', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_select_cpu(buck: Buck) -> None:
    """select() on config_setting(values={cpu: x86_64})."""
    result = await buck.build("//:select_cpu")
    output = result.get_build_report().output_for_target("//:select_cpu")
    content = output.read_text().strip()
    # CPU might or might not match x86_64 depending on host
    assert content in ("x86_64", "other_cpu"), f"Unexpected cpu value: '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_multi_select_concat(buck: Buck) -> None:
    """Multiple select() concatenation in same attribute."""
    result = await buck.build("//:multi_select_concat")
    output = result.get_build_report().output_for_target("//:multi_select_concat")
    content = output.read_text().strip()
    assert content == "default,not_dbg", f"Expected 'default,not_dbg', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_multi_select_concat_with_dbg(buck: Buck) -> None:
    """Multiple select() concatenation with --compilation_mode=dbg."""
    result = await buck.build("//:multi_select_concat", "--compilation_mode=dbg")
    output = result.get_build_report().output_for_target("//:multi_select_concat")
    content = output.read_text().strip()
    assert content == "default,dbg", f"Expected 'default,dbg', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_multi_select_concat_with_opt_and_dbg(buck: Buck) -> None:
    """Multiple select() with --compilation_mode=opt selects opt in first, not_dbg in second."""
    result = await buck.build("//:multi_select_concat", "--compilation_mode=opt")
    output = result.get_build_report().output_for_target("//:multi_select_concat")
    content = output.read_text().strip()
    assert content == "opt,not_dbg", f"Expected 'opt,not_dbg', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_select_deps_default(buck: Buck) -> None:
    """select() on deps: default has 1 dep (helper_lib)."""
    result = await buck.build("//:select_deps")
    output = result.get_build_report().output_for_target("//:select_deps")
    content = output.read_text().strip()
    assert content == "deps=1", f"Expected 'deps=1', got '{content}'"


@buck_test(data_dir="test_select_config_setting_data")
async def test_select_deps_opt(buck: Buck) -> None:
    """select() on deps: with --compilation_mode=opt has 2 deps (helper_lib + opt_lib)."""
    result = await buck.build("//:select_deps", "--compilation_mode=opt")
    output = result.get_build_report().output_for_target("//:select_deps")
    content = output.read_text().strip()
    assert content == "deps=2", f"Expected 'deps=2', got '{content}'"
