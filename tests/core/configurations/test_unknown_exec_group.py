# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# pyre-strict

# Plan 24 Phase 4 step 2: `actions.run(exec_group="<undeclared>")` must
# error with a clear message listing the valid (possibly empty) group
# names. Without this check, typos silently fall through and would
# surface much later as a "wrong platform" execution failure.


from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.asserts import expect_failure
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_unknown_exec_group_errors(buck: Buck) -> None:
    await expect_failure(
        buck.build("//:bad"),
        stderr_regex="exec group not declared on this rule",
    )
