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


@buck_test(data_dir="test_visibility_data")
async def test_visibility_public_builds(buck: Buck) -> None:
    """Targets with //visibility:public can be built."""
    result = await buck.build("//:always_public")
    output = result.get_build_report().output_for_target("//:always_public")
    assert "always" in output.read_text()


@buck_test(data_dir="test_visibility_data")
async def test_visibility_public_cross_package(buck: Buck) -> None:
    """Sub-package can depend on a target with //visibility:public visibility."""
    result = await buck.build("//sub:depends_on_public")
    output = result.get_build_report().output_for_target("//sub:depends_on_public")
    assert output.exists()


@buck_test(data_dir="test_visibility_data")
async def test_package_group_builds(buck: Buck) -> None:
    """package_group() targets can be defined and built."""
    await buck.build("//:my_packages")
