# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict


import os

from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_build_bazel_takes_precedence_over_build(buck: Buck) -> None:
    """Test that BUILD.bazel takes precedence over BUILD file.

    This tests Bazel-compatible build file detection:
    - When both BUILD.bazel and BUILD exist, BUILD.bazel should be used
    - When only BUILD exists, it should be used as fallback
    """
    # Initially, only BUILD.bazel exists - it should be used
    output = await buck.build("//:")
    assert "AAA from BUILD.bazel" in output.stderr
    assert "AAA from BUILD" not in output.stderr

    # Delete BUILD.bazel - now BUILD should be used as fallback
    os.unlink(buck.cwd / "BUILD.bazel")

    output = await buck.build("//:")
    assert "AAA from BUILD.bazel" not in output.stderr
    assert "AAA from BUILD" in output.stderr
