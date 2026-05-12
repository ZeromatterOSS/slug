# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# pyre-strict

# Plan 24 Phase 8: per-named-exec-group platform routing. With two
# registered platforms (linux + darwin) and a rule that declares two
# exec groups whose `exec_compatible_with` constraints partition the
# candidate list, each group must resolve to its matching platform.
#
# If per-group resolution is broken (every group falls back to the
# default candidate or to host alone), at least one group's constraints
# won't be satisfied and the resolver errors loudly. This test catches
# that regression by relying on a successful build to imply per-group
# matching worked.


from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_two_groups_pick_disjoint_platforms(buck: Buck) -> None:
    # Both platforms registered, rule declares one group constrained to
    # linux and one to darwin. The build can only succeed if the
    # `link` group picks linux_platform and the `test` group picks
    # darwin_platform — Phase 8's per-group routing.
    #
    # Fine-grained routing-correctness verification (asserting the
    # action's actual `re_properties` per group) lives in the Rust
    # unit tests in `slug_build_api::actions::registry::
    # select_action_executor_config_tests`. This integration test
    # only asserts the path doesn't error — equivalent to the
    # default-group `test_extra_exec_platforms` smoke check, but for
    # named groups.
    await buck.build(
        "//:fan",
        "--extra-execution-platforms=//:linux_platform",
        "--extra-execution-platforms=//:darwin_platform",
    )
