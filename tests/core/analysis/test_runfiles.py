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


@buck_test(data_dir="test_runfiles_data")
async def test_runfiles_collect(buck: Buck) -> None:
    result = await buck.build("//:collector")
    output = result.get_build_report().output_for_target("//:collector")

    content = output.read_text().strip().splitlines()
    assert content == ["data.txt", "dep.txt", "own.txt", "runtime.txt"]
