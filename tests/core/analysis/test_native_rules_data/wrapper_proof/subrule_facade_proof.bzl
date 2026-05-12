# Plan 28.4 Stage 5 acceptance fixture. Mirrors `facade_proof.bzl`
# (Stage 3) but for the subrule dispatch path. The rule impl invokes
# a `subrule()` defined in this file; the subrule's `ctx` should be
# the Stage-5-tagged facade installed by `_invoke_subrule` in
# `@slug_builtins//:exports.bzl`.
#
# The subrule impl checks:
#
#   1. `ctx.slug_facade_active == True` — proves the subrule wrapper
#      replaced raw_ctx with a struct facade.
#   2. `ctx.slug_facade_kind == "subrule"` — proves the SUBRULE
#      wrapper produced the facade, not the rule wrapper. (Without
#      this distinction, the Stage 3 facade leaking through TLS would
#      silently pass the active-marker check.)
#   3. `ctx.target_platform_has_constraint(...)` answers correctly
#      from inside a subrule. Same Starlark shim Stage 3/4 use.
#
# Subrule kwargs are forwarded verbatim: we pass a sentinel kwarg and
# assert it round-trips, to confirm `_invoke_subrule(implementation,
# raw_ctx, **kwargs)` doesn't drop or rewrite the caller's named args.

_HOST_OS_LABEL = "@platforms//os:linux"
_NON_HOST_OS_LABEL = "@platforms//os:windows"

def _facade_subrule_impl(ctx, *, sentinel):
    if not getattr(ctx, "slug_facade_active", False):
        fail("Plan 28.4 Stage 5: ctx.slug_facade_active missing inside subrule")
    kind = getattr(ctx, "slug_facade_kind", "")
    if kind != "subrule":
        fail("Plan 28.4 Stage 5: ctx.slug_facade_kind = %r (want \"subrule\")" % kind)
    if sentinel != "kwarg-from-rule":
        fail("Plan 28.4 Stage 5: subrule kwarg sentinel was %r, expected \"kwarg-from-rule\"" % sentinel)

    matching = platform_common.ConstraintValueInfo(
        label = _HOST_OS_LABEL,
        constraint_setting = "@platforms//os:os",
    )
    non_matching = platform_common.ConstraintValueInfo(
        label = _NON_HOST_OS_LABEL,
        constraint_setting = "@platforms//os:os",
    )
    if not ctx.target_platform_has_constraint(matching):
        fail("Plan 28.4 Stage 5: target_platform_has_constraint False for host inside subrule")
    if ctx.target_platform_has_constraint(non_matching):
        fail("Plan 28.4 Stage 5: target_platform_has_constraint True for non-host inside subrule")
    return struct(ok = True)

facade_subrule = subrule(implementation = _facade_subrule_impl)

def _subrule_facade_target_impl(ctx):
    result = facade_subrule(sentinel = "kwarg-from-rule")
    if not result.ok:
        fail("Plan 28.4 Stage 5: subrule did not report ok")
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "subrule-facade-proof-ok\n")
    return [DefaultInfo(default_output = out)]

subrule_facade_target = rule(
    implementation = _subrule_facade_target_impl,
    attrs = {},
    subrules = [facade_subrule],
)
