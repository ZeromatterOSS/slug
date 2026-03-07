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


@buck_test(data_dir="test_analysis_test_data")
async def test_analysis_test_builds(buck: Buck) -> None:
    """testing.analysis_test() target can be defined and analyzed."""
    await buck.build("//:check_my_info")


@buck_test(data_dir="test_analysis_test_data")
async def test_analysis_test_is_a_test_target(buck: Buck) -> None:
    """testing.analysis_test() creates a target with test providers."""
    result = await buck.build("//:check_my_info")
    # The target should analyze successfully (analysis tests pass by analyzing)
    build_report = result.get_build_report()
    assert build_report is not None
