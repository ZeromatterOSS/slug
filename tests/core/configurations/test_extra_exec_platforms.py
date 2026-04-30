# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree.

# pyre-strict

# Plan 24 Phase 3: end-to-end verification of `--extra_execution_platforms`
# constraint matching. With two registered platforms (linux + darwin) the
# resolver must pick the one whose constraint_values satisfy the target's
# `exec_compatible_with`, regardless of the order they were passed on the
# command line. Without Phase 1's candidate surfacing, both targets would
# fall through to the legacy host platform and silently ignore
# `exec_compatible_with`.


from buck2.tests.e2e_util.api.buck import Buck
from buck2.tests.e2e_util.asserts import expect_failure
from buck2.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_linux_target_picks_linux_platform(buck: Buck) -> None:
    # Both platforms registered; darwin first so the resolver can't just
    # pick "the first one" — it has to honor the constraint.
    await buck.build(
        "//:needs_linux",
        "--extra-execution-platforms=//:darwin_platform",
        "--extra-execution-platforms=//:linux_platform",
    )


@buck_test()
async def test_darwin_target_picks_darwin_platform(buck: Buck) -> None:
    # Same flag order — darwin first — but the target wants darwin, so
    # darwin_platform must win. Demonstrates that flag order does NOT
    # determine selection when constraints are present.
    await buck.build(
        "//:needs_darwin",
        "--extra-execution-platforms=//:darwin_platform",
        "--extra-execution-platforms=//:linux_platform",
    )


@buck_test()
async def test_no_compatible_platform_errors_loudly(buck: Buck) -> None:
    # Only darwin registered; the linux target has no compatible platform.
    # Plan 24 sets `Fallback::Error` when registrations exist (no silent
    # host fallback), so this must error rather than succeed quietly.
    await expect_failure(
        buck.build(
            "//:needs_linux",
            "--extra-execution-platforms=//:darwin_platform",
        ),
        stderr_regex="No compatible execution platform",
    )


@buck_test()
async def test_error_lists_all_skipped_platforms(buck: Buck) -> None:
    # Plan 24 Phase 5: when constraints match no candidate, the error
    # must enumerate every registered platform with the reason it was
    # skipped — so a user can see, at a glance, the full attempted set
    # rather than just the first miss.
    failure = await expect_failure(
        buck.build(
            "//:needs_fuchsia",
            "--extra-execution-platforms=//:linux_platform",
            "--extra-execution-platforms=//:darwin_platform",
        ),
        stderr_regex="No compatible execution platform",
    )
    assert "linux_platform" in failure.stderr, (
        f"error must name linux_platform as skipped, got:\n{failure.stderr}"
    )
    assert "darwin_platform" in failure.stderr, (
        f"error must name darwin_platform as skipped, got:\n{failure.stderr}"
    )
    assert "fuchsia" in failure.stderr, (
        f"error must name the unmatched constraint, got:\n{failure.stderr}"
    )
