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


@buck_test(data_dir="test_aspects_data")
async def test_aspect_basic_propagation(buck: Buck) -> None:
    """Aspect propagates through deps and collects files transitively."""
    result = await buck.build("//:collect_bar")
    output = result.get_build_report().output_for_target("//:collect_bar")
    content = output.read_text()
    # bar depends on foo, so both bar.txt and foo.txt should appear
    assert "bar.txt" in content, f"Expected bar.txt in output: {content!r}"
    assert "foo.txt" in content, f"Expected foo.txt in output: {content!r}"


@buck_test(data_dir="test_aspects_data")
async def test_aspect_transitive_propagation(buck: Buck) -> None:
    """Aspect propagates transitively through a chain: c -> b -> a."""
    result = await buck.build("//:collect_c")
    output = result.get_build_report().output_for_target("//:collect_c")
    content = output.read_text()
    # c depends on b which depends on a, so all three should appear
    assert "a.txt" in content, f"Expected a.txt in output: {content!r}"
    assert "b.txt" in content, f"Expected b.txt in output: {content!r}"
    assert "c.txt" in content, f"Expected c.txt in output: {content!r}"


@buck_test(data_dir="test_aspects_data")
async def test_aspect_provider_access(buck: Buck) -> None:
    """Aspects can access providers from their target (shadow graph)."""
    result = await buck.build("//:collect_bar")
    # Should successfully build without errors
    output = result.get_build_report().output_for_target("//:collect_bar")
    assert output.exists()


@buck_test(data_dir="test_aspects_data")
async def test_aspect_required_providers_filter(buck: Buck) -> None:
    """Aspect with required_providers only runs on targets that provide those providers."""
    result = await buck.build("//:count_mixed")
    output = result.get_build_report().output_for_target("//:count_mixed")
    content = output.read_text()
    lines = content.strip().splitlines()
    # :tagged has TagInfo -> aspect applies -> "tagged:1"
    # :plain has no TagInfo -> aspect skipped -> "untagged"
    assert "tagged:1" in lines, f"Expected 'tagged:1' in {lines}"
    assert "untagged" in lines, f"Expected 'untagged' in {lines}"


@buck_test(data_dir="test_aspects_data")
async def test_aspect_ctx_rule_kind(buck: Buck) -> None:
    """ctx.rule.kind returns the rule type name of the visited target."""
    result = await buck.build("//:kinds_of_deps")
    output = result.get_build_report().output_for_target("//:kinds_of_deps")
    content = output.read_text()
    kinds = content.strip().splitlines()
    # :foo is a my_lib, :tagged is a tagged_lib
    assert "my_lib" in kinds, f"Expected 'my_lib' in {kinds}"
    assert "tagged_lib" in kinds, f"Expected 'tagged_lib' in {kinds}"
