# Plan 28.4 Stage 3 acceptance fixture. Two checks pinned in one rule:
#
#   1. `ctx.kuro_facade_active == True`. The marker is set by the
#      Starlark facade in `@kuro_builtins//:exports.bzl::_invoke_rule`.
#      A rule reaching this point with the marker missing means the
#      wrapper was bypassed or returned `raw_ctx` directly — Stage 3
#      regressed back to Stage 2 behaviour.
#
#   2. `ctx.target_platform_has_constraint(...)` is served by Starlark.
#      The Rust impl was deleted as part of Stage 3, so any answer at
#      all is proof the migration landed. To make the test meaningful
#      we additionally pin a positive case (a label the host should
#      match) and a negative case (a label no host can match), both
#      derived from the host-shortcut table in the Rust source we
#      removed. Built only on Linux per the buck_test runner; if Stage
#      4+ adds macOS coverage, expand the matching set.
#
# `platform_common.ConstraintValueInfo` is callable and exposes a
# `.label` attribute — see `_constraint_provider_test_impl` in
# `tests/core/analysis/test_native_rules_data/defs.bzl`.

_HOST_OS_LABEL = "@platforms//os:linux"
_NON_HOST_OS_LABEL = "@platforms//os:windows"

def _facade_proof_impl(ctx):
    if not getattr(ctx, "kuro_facade_active", False):
        fail("Plan 28.4 Stage 3: ctx.kuro_facade_active missing — wrapper not in call path")

    matching = platform_common.ConstraintValueInfo(
        label = _HOST_OS_LABEL,
        constraint_setting = "@platforms//os:os",
    )
    non_matching = platform_common.ConstraintValueInfo(
        label = _NON_HOST_OS_LABEL,
        constraint_setting = "@platforms//os:os",
    )

    if not ctx.target_platform_has_constraint(matching):
        fail("Plan 28.4 Stage 3: target_platform_has_constraint returned False for host OS %s" % _HOST_OS_LABEL)
    if ctx.target_platform_has_constraint(non_matching):
        fail("Plan 28.4 Stage 3: target_platform_has_constraint returned True for non-host OS %s" % _NON_HOST_OS_LABEL)

    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "facade-proof-ok\n")
    return [DefaultInfo(default_output = out)]

facade_proof = rule(
    implementation = _facade_proof_impl,
    attrs = {},
)
